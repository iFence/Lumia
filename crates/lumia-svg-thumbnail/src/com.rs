//! Hand-rolled COM plumbing for the SVG thumbnail provider.
//!
//! Windows ships no built-in SVG thumbnail support, so Explorer loads this DLL
//! in-process (`explorer.exe` for small thumbnails, `dllhost.exe` for large
//! ones) through the `IThumbnailProvider` interface. The shell feeds us the
//! file contents as an `IStream` via `IInitializeWithStream`, then calls
//! `GetThumbnail` for the pixel data.
//!
//! `windows-sys` exposes raw Win32 signatures but no COM interface vtables, so
//! the few interfaces we need (a client view of `IStream`, our provider, and
//! its `IClassFactory`) are declared by hand. Every entry point is defensive:
//! `GetThumbnail` runs inside `catch_unwind` and any failure returns `E_FAIL`,
//! letting the shell fall back to the default file icon rather than crashing
//! the shell process.

use std::cell::RefCell;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_NOINTERFACE, S_FALSE, S_OK,
};
use windows_sys::Win32::Graphics::Gdi::HBITMAP;
use windows_sys::Win32::UI::Shell::WTS_ALPHATYPE;
use windows_sys::core::{IID_IUnknown, GUID, HRESULT};

use crate::{dib, render};

/// Our COM class id. Lumia registers this CLSID in the registry; the shell
/// `CoCreateInstance`s it whenever it needs a thumbnail for `.svg`/`.svgz`.
const CLSID_SVG_THUMBNAIL: GUID = GUID::from_u128(0x0f6f22c8_3077_4b32_a61c_7738e61f242b);

const IID_ICLASS_FACTORY: GUID = GUID::from_u128(0x00000001_0000_0000_c000_000000000046);
const IID_IINITIALIZE_WITH_STREAM: GUID = GUID::from_u128(0xb824b49d_22ac_4161_ac8a_9916e8fa3f7f);
const IID_ITHUMBNAIL_PROVIDER: GUID = GUID::from_u128(0xe357fccd_a995_4576_b01f_234630154e96);

/// Refuse to buffer more than this many SVG bytes coming from a stream.
const MAX_SVG_BYTES: usize = 16 * 1024 * 1024;

/// Outstanding objects (providers + class factories) and `LockServer` holds.
/// Both must reach zero before the shell may unload the DLL.
static OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);
static LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

fn guid_eq(left: &GUID, right: &GUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

/// COM entry point: hand out an `IClassFactory` for our CLSID.
#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // SAFETY: the shell passes valid pointers for `rclsid`, `riid`, `ppv`.
    unsafe {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_FAIL;
        }
        if !guid_eq(&*rclsid, &CLSID_SVG_THUMBNAIL) {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory = Box::into_raw(ClassFactory::new());
        let result = class_factory_query_interface(factory, &*riid, ppv);
        if result != S_OK {
            OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
            drop(Box::from_raw(factory));
        }
        result
    }
}

/// The DLL may unload once no objects or class-factory locks are outstanding.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECT_COUNT.load(Ordering::SeqCst) == 0 && LOCK_COUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

// --- Client-side `IStream` -------------------------------------------------

type IStreamRead = unsafe extern "system" fn(
    stream: *mut core::ffi::c_void,
    pv: *mut core::ffi::c_void,
    cb: u32,
    pcb_read: *mut u32,
) -> HRESULT;

/// The portion of `IStream`'s vtable we touch. `Read` is slot 3, right after
/// the three `IUnknown` methods; the remaining ten slots are never called.
#[repr(C)]
struct IStreamVtbl {
    query_interface: *const core::ffi::c_void,
    add_ref: *const core::ffi::c_void,
    release: *const core::ffi::c_void,
    read: IStreamRead,
    _rest: [*const core::ffi::c_void; 10],
}

/// Read the entire contents of `stream` into memory, bailing out past
/// `max_bytes`.
fn read_stream_to_end(stream: *mut core::ffi::c_void, max_bytes: usize) -> Result<Vec<u8>, ()> {
    // SAFETY: the shell hands us a valid `IStream*`; its first member is the
    // vtable pointer, whose `Read` slot we call below.
    let vtable = unsafe { &**(stream as *const *const IStreamVtbl) };
    let read = vtable.read;

    let mut contents = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let mut got = 0u32;
        // SAFETY: `stream` and the buffer pointer are valid for the call; the
        // callee writes at most `chunk.len()` bytes and reports `got`.
        let result =
            unsafe { read(stream, chunk.as_mut_ptr() as *mut core::ffi::c_void, chunk.len() as u32, &mut got) };
        if got == 0 {
            // EOF, or an error reported without bytes — either way we are done.
            break;
        }
        if result != S_OK {
            return Err(());
        }
        contents.extend_from_slice(&chunk[..got as usize]);
        if contents.len() > max_bytes {
            return Err(());
        }
    }
    Ok(contents)
}

// --- `ThumbnailProvider` object --------------------------------------------

struct ThumbnailProvider {
    // The shell reads the vtable pointer through the raw COM object pointer;
    // safe Rust never touches this field, so silence the dead-code lint.
    #[allow(dead_code)]
    vtable: &'static ThumbnailProviderVtbl,
    ref_count: AtomicU32,
    svg_bytes: RefCell<Option<Vec<u8>>>,
}

/// Combined vtable: `IUnknown` (slots 0-2), `IInitializeWithStream::Initialize`
/// (slot 3), then `IThumbnailProvider::GetThumbnail` (slot 4).
#[repr(C)]
struct ThumbnailProviderVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut ThumbnailProvider,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> HRESULT,
    add_ref: unsafe extern "system" fn(this: *mut ThumbnailProvider) -> u32,
    release: unsafe extern "system" fn(this: *mut ThumbnailProvider) -> u32,
    initialize: unsafe extern "system" fn(
        this: *mut ThumbnailProvider,
        stream: *mut core::ffi::c_void,
        grf_mode: u32,
    ) -> HRESULT,
    get_thumbnail: unsafe extern "system" fn(
        this: *mut ThumbnailProvider,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdw_alpha: *mut WTS_ALPHATYPE,
    ) -> HRESULT,
}

static THUMBNAIL_PROVIDER_VTABLE: ThumbnailProviderVtbl = ThumbnailProviderVtbl {
    query_interface: thumbnail_query_interface,
    add_ref: thumbnail_add_ref,
    release: thumbnail_release,
    initialize: thumbnail_initialize,
    get_thumbnail: thumbnail_get_thumbnail,
};

impl ThumbnailProvider {
    fn new() -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            vtable: &THUMBNAIL_PROVIDER_VTABLE,
            ref_count: AtomicU32::new(1),
            svg_bytes: RefCell::new(None),
        })
    }
}

unsafe extern "system" fn thumbnail_query_interface(
    this: *mut ThumbnailProvider,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // SAFETY: `this`, `riid`, and `ppv` are valid COM call arguments.
    unsafe {
        let iid = &*riid;
        if guid_eq(iid, &IID_IUnknown)
            || guid_eq(iid, &IID_IINITIALIZE_WITH_STREAM)
            || guid_eq(iid, &IID_ITHUMBNAIL_PROVIDER)
        {
            (*this).ref_count.fetch_add(1, Ordering::SeqCst);
            *ppv = this as *mut core::ffi::c_void;
            S_OK
        } else {
            *ppv = ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn thumbnail_add_ref(this: *mut ThumbnailProvider) -> u32 {
    // SAFETY: `this` is a valid `ThumbnailProvider*` while the caller holds a
    // reference.
    unsafe { (*this).ref_count.fetch_add(1, Ordering::SeqCst) + 1 }
}

unsafe extern "system" fn thumbnail_release(this: *mut ThumbnailProvider) -> u32 {
    // SAFETY: see `thumbnail_add_ref`; this is the matching release.
    let remaining = unsafe { (*this).ref_count.fetch_sub(1, Ordering::SeqCst) - 1 };
    if remaining == 0 {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        // SAFETY: the object was created with `Box::into_raw` and its last
        // reference is now gone.
        unsafe { drop(Box::from_raw(this)) };
    }
    remaining
}

unsafe extern "system" fn thumbnail_initialize(
    this: *mut ThumbnailProvider,
    stream: *mut core::ffi::c_void,
    _grf_mode: u32,
) -> HRESULT {
    if stream.is_null() {
        return E_FAIL;
    }
    let bytes = match read_stream_to_end(stream, MAX_SVG_BYTES) {
        Ok(bytes) => bytes,
        Err(()) => return E_FAIL,
    };
    // SAFETY: `this` is a valid `ThumbnailProvider*` while the caller holds a
    // reference, so the `RefCell` is accessible. `try_borrow_mut` never panics.
    unsafe {
        match (*this).svg_bytes.try_borrow_mut() {
            Ok(mut slot) => {
                *slot = Some(bytes);
                S_OK
            }
            Err(_) => E_FAIL,
        }
    }
}

unsafe extern "system" fn thumbnail_get_thumbnail(
    this: *mut ThumbnailProvider,
    cx: u32,
    phbmp: *mut HBITMAP,
    pdw_alpha: *mut WTS_ALPHATYPE,
) -> HRESULT {
    if phbmp.is_null() || pdw_alpha.is_null() {
        return E_FAIL;
    }
    // Never let a panic cross the FFI boundary: a crash in this DLL would take
    // down the whole shell process.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: `this`, `phbmp`, and `pdw_alpha` are valid COM call args.
        unsafe {
            let bytes = match (*this).svg_bytes.try_borrow() {
                Ok(bytes) => bytes,
                Err(_) => return E_FAIL,
            };
            let Some(bytes) = bytes.as_deref() else {
                return E_FAIL;
            };
            let rendered = match render::svg_bytes_to_rgba(bytes, cx) {
                Ok(rendered) => rendered,
                Err(_) => return E_FAIL,
            };
            let (bitmap, alpha_type) = match dib::rgba_to_hbitmap(
                rendered.width,
                rendered.height,
                &rendered.premultiplied_rgba,
            ) {
                Ok(result) => result,
                Err(()) => return E_FAIL,
            };
            *phbmp = bitmap;
            *pdw_alpha = alpha_type;
            S_OK
        }
    }));
    result.unwrap_or(E_FAIL)
}

// --- `IClassFactory` object -------------------------------------------------

struct ClassFactory {
    // See `ThumbnailProvider::vtable`.
    #[allow(dead_code)]
    vtable: &'static ClassFactoryVtbl,
    ref_count: AtomicU32,
}

#[repr(C)]
struct ClassFactoryVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut ClassFactory,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> HRESULT,
    add_ref: unsafe extern "system" fn(this: *mut ClassFactory) -> u32,
    release: unsafe extern "system" fn(this: *mut ClassFactory) -> u32,
    create_instance: unsafe extern "system" fn(
        this: *mut ClassFactory,
        outer: *mut core::ffi::c_void,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> HRESULT,
    lock_server: unsafe extern "system" fn(this: *mut ClassFactory, f_lock: i32) -> HRESULT,
}

static CLASS_FACTORY_VTABLE: ClassFactoryVtbl = ClassFactoryVtbl {
    query_interface: class_factory_query_interface,
    add_ref: class_factory_add_ref,
    release: class_factory_release,
    create_instance: class_factory_create_instance,
    lock_server: class_factory_lock_server,
};

impl ClassFactory {
    fn new() -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            vtable: &CLASS_FACTORY_VTABLE,
            ref_count: AtomicU32::new(1),
        })
    }
}

unsafe extern "system" fn class_factory_query_interface(
    this: *mut ClassFactory,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // SAFETY: valid COM call arguments, as in `thumbnail_query_interface`.
    unsafe {
        let iid = &*riid;
        if guid_eq(iid, &IID_IUnknown) || guid_eq(iid, &IID_ICLASS_FACTORY) {
            (*this).ref_count.fetch_add(1, Ordering::SeqCst);
            *ppv = this as *mut core::ffi::c_void;
            S_OK
        } else {
            *ppv = ptr::null_mut();
            E_NOINTERFACE
        }
    }
}

unsafe extern "system" fn class_factory_add_ref(this: *mut ClassFactory) -> u32 {
    // SAFETY: `this` is a valid `ClassFactory*` while the caller holds a
    // reference.
    unsafe { (*this).ref_count.fetch_add(1, Ordering::SeqCst) + 1 }
}

unsafe extern "system" fn class_factory_release(this: *mut ClassFactory) -> u32 {
    // SAFETY: see `class_factory_add_ref`.
    let remaining = unsafe { (*this).ref_count.fetch_sub(1, Ordering::SeqCst) - 1 };
    if remaining == 0 {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
        // SAFETY: the factory was created with `Box::into_raw` and its last
        // reference is now gone.
        unsafe { drop(Box::from_raw(this)) };
    }
    remaining
}

unsafe extern "system" fn class_factory_create_instance(
    _this: *mut ClassFactory,
    outer: *mut core::ffi::c_void,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // Aggregation is not supported.
    if !outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    // SAFETY: valid COM call arguments; `riid`/`ppv` mirror the class factory.
    unsafe {
        let provider = Box::into_raw(ThumbnailProvider::new());
        let result = thumbnail_query_interface(provider, &*riid, ppv);
        if result == S_OK {
            // Hand the caller its reference and drop the factory's initial one
            // so the object dies when the caller releases it.
            thumbnail_release(provider);
        } else {
            OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
            drop(Box::from_raw(provider));
        }
        result
    }
}

unsafe extern "system" fn class_factory_lock_server(_this: *mut ClassFactory, f_lock: i32) -> HRESULT {
    if f_lock != 0 {
        LOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        LOCK_COUNT.fetch_sub(1, Ordering::SeqCst);
    }
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_factory(ppv: *mut core::ffi::c_void) {
        let factory = ppv as *mut ClassFactory;
        // SAFETY: `ppv` came from `DllGetClassObject`, which returned a factory
        // with a single reference owned by the caller.
        unsafe { class_factory_release(factory) };
    }

    #[test]
    fn rejects_unknown_clsid() {
        let unknown = GUID::from_u128(0x11111111_2222_3333_4444_555555555555);
        let mut ppv = ptr::null_mut();
        let result = DllGetClassObject(&unknown, &IID_ICLASS_FACTORY, &mut ppv);
        assert_eq!(result, CLASS_E_CLASSNOTAVAILABLE);
        assert!(ppv.is_null());
    }

    #[test]
    fn returns_class_factory_for_known_clsid() {
        let mut ppv = ptr::null_mut();
        let result = DllGetClassObject(&CLSID_SVG_THUMBNAIL, &IID_ICLASS_FACTORY, &mut ppv);
        assert_eq!(result, S_OK);
        assert!(!ppv.is_null());
        release_factory(ppv);
    }

    #[test]
    fn factory_creates_provider_and_rejects_aggregation() {
        let mut ppv = ptr::null_mut();
        let result = DllGetClassObject(&CLSID_SVG_THUMBNAIL, &IID_ICLASS_FACTORY, &mut ppv);
        assert_eq!(result, S_OK);
        let factory = ppv as *mut ClassFactory;

        // Creating a provider object returns a live `IThumbnailProvider`.
        let mut provider = ptr::null_mut();
        let result = unsafe {
            class_factory_create_instance(
                factory,
                ptr::null_mut(),
                &IID_ITHUMBNAIL_PROVIDER,
                &mut provider,
            )
        };
        assert_eq!(result, S_OK);
        assert!(!provider.is_null());
        // SAFETY: `provider` is the caller's reference on the object.
        unsafe { thumbnail_release(provider as *mut ThumbnailProvider) };

        // Aggregation is not supported.
        let mut rejected = ptr::null_mut();
        let result = unsafe {
            class_factory_create_instance(
                factory,
                1 as *mut core::ffi::c_void,
                &IID_ITHUMBNAIL_PROVIDER,
                &mut rejected,
            )
        };
        assert_eq!(result, CLASS_E_NOAGGREGATION);
        assert!(rejected.is_null());

        release_factory(ppv);
    }

    #[test]
    fn provider_surfaces_all_interfaces() {
        let mut ppv = ptr::null_mut();
        let result = DllGetClassObject(&CLSID_SVG_THUMBNAIL, &IID_ICLASS_FACTORY, &mut ppv);
        assert_eq!(result, S_OK);
        let factory = ppv as *mut ClassFactory;

        let mut provider = ptr::null_mut();
        let result = unsafe {
            class_factory_create_instance(
                factory,
                ptr::null_mut(),
                &IID_IINITIALIZE_WITH_STREAM,
                &mut provider,
            )
        };
        assert_eq!(result, S_OK);

        // QueryInterface round-trips to `IThumbnailProvider` on the same object.
        let provider_ptr = provider as *mut ThumbnailProvider;
        let mut provider_interface = ptr::null_mut();
        let result = unsafe {
            thumbnail_query_interface(provider_ptr, &IID_ITHUMBNAIL_PROVIDER, &mut provider_interface)
        };
        assert_eq!(result, S_OK);
        assert_eq!(provider_interface, provider);
        // SAFETY: both references are released below.
        unsafe { thumbnail_release(provider_interface as *mut ThumbnailProvider) };
        unsafe { thumbnail_release(provider_ptr) };

        release_factory(ppv);
    }
}
