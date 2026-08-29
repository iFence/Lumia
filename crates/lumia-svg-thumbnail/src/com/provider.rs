//! The `ThumbnailProvider` COM object: an `IUnknown` implementation that also
//! exposes `IInitializeWithStream` and `IThumbnailProvider`. Split from
//! `com.rs`, which keeps the DLL entry points and the class factory.
//!
//! `IInitializeWithStream::Initialize` and `IThumbnailProvider::GetThumbnail`
//! both sit at vtable slot 3 (each interface starts with IUnknown's slots
//! 0-2), so a single combined vtable cannot serve both. The object therefore
//! carries one vtable per interface.

use std::cell::RefCell;
use std::mem::offset_of;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_sys::core::{IID_IUnknown, GUID, HRESULT};
use windows_sys::Win32::Foundation::{E_FAIL, E_NOINTERFACE, S_OK};
use windows_sys::Win32::Graphics::Gdi::HBITMAP;
use windows_sys::Win32::UI::Shell::WTS_ALPHATYPE;

use super::{
    guid_eq, read_stream_to_end, IID_IINITIALIZE_WITH_STREAM, IID_ITHUMBNAIL_PROVIDER,
    MAX_SVG_BYTES, OBJECT_COUNT,
};
use crate::{dib, render};

// --- `ThumbnailProvider` object --------------------------------------------

pub(super) struct ThumbnailProvider {
    // The shell reads the vtable pointer through the raw COM object pointer;
    // safe Rust never touches these fields, so silence the dead-code lint.
    //
    // `IInitializeWithStream::Initialize` and `IThumbnailProvider::GetThumbnail`
    // both sit at vtable slot 3 (each interface starts with IUnknown's slots
    // 0-2), so a single combined vtable cannot serve both. The object instead
    // exposes two interface pointers, each with its own vtable:
    //   * the base pointer (offset 0) is `IInitializeWithStream`;
    //   * `IThumbnailProvider` is reached through `provider_vtable` at a fixed
    //     offset, and its IUnknown/`GetThumbnail` methods convert back to the
    //     base pointer before operating.
    #[allow(dead_code)]
    init_vtable: &'static IInitializeWithStreamVtbl,
    #[allow(dead_code)]
    provider_vtable: &'static ThumbnailProviderVtbl,
    ref_count: AtomicU32,
    svg_bytes: RefCell<Option<Vec<u8>>>,
}

/// `IInitializeWithStream` vtable: `IUnknown` (slots 0-2), then `Initialize`
/// (slot 3). The shell's first contact with the provider goes through this
/// interface pointer (the base object).
#[repr(C)]
struct IInitializeWithStreamVtbl {
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
}

/// `IThumbnailProvider` vtable: `IUnknown` (slots 0-2), then `GetThumbnail`
/// (slot 3). `this` is the interface pointer (the address of the
/// `provider_vtable` field), not the base object pointer.
#[repr(C)]
struct ThumbnailProviderVtbl {
    query_interface: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        riid: *const GUID,
        ppv: *mut *mut core::ffi::c_void,
    ) -> HRESULT,
    add_ref: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    release: unsafe extern "system" fn(this: *mut core::ffi::c_void) -> u32,
    get_thumbnail: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdw_alpha: *mut WTS_ALPHATYPE,
    ) -> HRESULT,
}

static IINITIALIZE_WITH_STREAM_VTABLE: IInitializeWithStreamVtbl = IInitializeWithStreamVtbl {
    query_interface: thumbnail_query_interface,
    add_ref: thumbnail_add_ref,
    release: thumbnail_release,
    initialize: thumbnail_initialize,
};

static THUMBNAIL_PROVIDER_VTABLE: ThumbnailProviderVtbl = ThumbnailProviderVtbl {
    query_interface: provider_query_interface,
    add_ref: provider_add_ref,
    release: provider_release,
    get_thumbnail: provider_get_thumbnail,
};

impl ThumbnailProvider {
    pub(super) fn new() -> Box<Self> {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Box::new(Self {
            init_vtable: &IINITIALIZE_WITH_STREAM_VTABLE,
            provider_vtable: &THUMBNAIL_PROVIDER_VTABLE,
            ref_count: AtomicU32::new(1),
            svg_bytes: RefCell::new(None),
        })
    }
}

/// Recover the base `ThumbnailProvider` from an `IThumbnailProvider` interface
/// pointer, which points at the `provider_vtable` field.
unsafe fn provider_to_base(provider_ptr: *mut core::ffi::c_void) -> *mut ThumbnailProvider {
    // SAFETY: `provider_ptr` was produced by `thumbnail_query_interface` as the
    // address of the `provider_vtable` field, so subtracting its offset lands
    // on the base object.
    unsafe {
        let offset = offset_of!(ThumbnailProvider, provider_vtable) as isize;
        (provider_ptr as *mut u8).offset(-offset) as *mut ThumbnailProvider
    }
}

pub(super) unsafe extern "system" fn thumbnail_query_interface(
    this: *mut ThumbnailProvider,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // SAFETY: `this`, `riid`, and `ppv` are valid COM call arguments.
    unsafe {
        let iid = &*riid;
        if guid_eq(iid, &IID_IUnknown) || guid_eq(iid, &IID_IINITIALIZE_WITH_STREAM) {
            (*this).ref_count.fetch_add(1, Ordering::SeqCst);
            *ppv = this as *mut core::ffi::c_void;
            S_OK
        } else if guid_eq(iid, &IID_ITHUMBNAIL_PROVIDER) {
            (*this).ref_count.fetch_add(1, Ordering::SeqCst);
            // The two interfaces disagree on slot 3, so hand back a distinct
            // pointer into the object carrying `IThumbnailProvider`'s vtable.
            *ppv = ptr::addr_of_mut!((*this).provider_vtable) as *mut core::ffi::c_void;
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

pub(super) unsafe extern "system" fn thumbnail_release(this: *mut ThumbnailProvider) -> u32 {
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

// --- `IThumbnailProvider` interface ----------------------------------------

// These methods receive the `IThumbnailProvider` interface pointer, which is
// the address of the `provider_vtable` field rather than the base object. They
// convert back to the base and forward to the shared implementations above.

pub(super) unsafe extern "system" fn provider_query_interface(
    provider_ptr: *mut core::ffi::c_void,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // SAFETY: `provider_ptr` is a valid interface pointer handed out by
    // `thumbnail_query_interface`.
    unsafe { thumbnail_query_interface(provider_to_base(provider_ptr), riid, ppv) }
}

unsafe extern "system" fn provider_add_ref(provider_ptr: *mut core::ffi::c_void) -> u32 {
    // SAFETY: see `provider_query_interface`.
    unsafe { thumbnail_add_ref(provider_to_base(provider_ptr)) }
}

pub(super) unsafe extern "system" fn provider_release(provider_ptr: *mut core::ffi::c_void) -> u32 {
    // SAFETY: see `provider_query_interface`; the pointer stays valid while the
    // caller holds a reference.
    unsafe { thumbnail_release(provider_to_base(provider_ptr)) }
}

unsafe extern "system" fn provider_get_thumbnail(
    provider_ptr: *mut core::ffi::c_void,
    cx: u32,
    phbmp: *mut HBITMAP,
    pdw_alpha: *mut WTS_ALPHATYPE,
) -> HRESULT {
    // SAFETY: see `provider_query_interface`; `cx`/`phbmp`/`pdw_alpha` mirror
    // the arguments of `thumbnail_get_thumbnail`.
    unsafe { thumbnail_get_thumbnail(provider_to_base(provider_ptr), cx, phbmp, pdw_alpha) }
}
