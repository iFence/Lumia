use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use exif::{Exif, Field, Rational, Tag, Value};

use super::ExifMetadata;

pub(super) fn read_exif_metadata(path: &Path) -> ExifMetadata {
    let chroma_subsampling = read_jpeg_chroma_subsampling(path);
    let Ok(file) = File::open(path) else {
        return ExifMetadata {
            chroma_subsampling,
            ..ExifMetadata::default()
        };
    };
    let mut reader = BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else {
        return ExifMetadata {
            chroma_subsampling,
            ..ExifMetadata::default()
        };
    };

    ExifMetadata {
        chroma_subsampling,
        color_space: color_space(&exif),
        camera_make: ascii_field(&exif, Tag::Make),
        camera_model: ascii_field(&exif, Tag::Model),
        software: ascii_field(&exif, Tag::Software),
        lens: ascii_field(&exif, Tag::LensModel),
        date_taken: ascii_field(&exif, Tag::DateTimeOriginal).map(format_exif_datetime),
        flash: unsigned_field(&exif, Tag::Flash)
            .map(|value| if value & 1 == 1 { "Yes" } else { "No" }.to_owned()),
        focal_length: rational_field(&exif, Tag::FocalLength).map(|value| format!("{value:.2}mm")),
        exposure_time: exposure_time(&exif),
        exposure_bias: signed_rational_field(&exif, Tag::ExposureBiasValue)
            .map(|value| format!("{value:.2} EV")),
        aperture: rational_field(&exif, Tag::FNumber).map(|value| format!("f/{value:.1}")),
        iso: unsigned_field(&exif, Tag::PhotographicSensitivity).map(|value| value.to_string()),
        exposure_program: unsigned_field(&exif, Tag::ExposureProgram).map(exposure_program),
        metering_mode: unsigned_field(&exif, Tag::MeteringMode).map(metering_mode),
        gps: gps(&exif),
    }
}

fn field(exif: &Exif, tag: Tag) -> Option<&Field> {
    exif.fields().find(|field| field.tag == tag)
}

fn ascii_field(exif: &Exif, tag: Tag) -> Option<String> {
    let Value::Ascii(values) = &field(exif, tag)?.value else {
        return None;
    };
    let value = values.first()?;
    let end = value
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(value.len());
    let value = String::from_utf8_lossy(&value[..end]).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn unsigned_field(exif: &Exif, tag: Tag) -> Option<u32> {
    match &field(exif, tag)?.value {
        Value::Byte(values) => values.first().copied().map(u32::from),
        Value::Short(values) => values.first().copied().map(u32::from),
        Value::Long(values) => values.first().copied(),
        _ => None,
    }
}

fn rational_field(exif: &Exif, tag: Tag) -> Option<f64> {
    let Value::Rational(values) = &field(exif, tag)?.value else {
        return None;
    };
    rational_value(*values.first()?)
}

fn signed_rational_field(exif: &Exif, tag: Tag) -> Option<f64> {
    let Value::SRational(values) = &field(exif, tag)?.value else {
        return None;
    };
    let value = values.first()?;
    (value.denom != 0).then(|| f64::from(value.num) / f64::from(value.denom))
}

fn rational_value(value: Rational) -> Option<f64> {
    (value.denom != 0).then(|| f64::from(value.num) / f64::from(value.denom))
}

fn format_exif_datetime(value: String) -> String {
    let bytes = value.as_bytes();
    if bytes.len() >= 10 && bytes[4] == b':' && bytes[7] == b':' {
        format!(
            "{}/{}/{}{}",
            &value[..4],
            &value[5..7],
            &value[8..10],
            &value[10..]
        )
    } else {
        value
    }
}

fn exposure_time(exif: &Exif) -> Option<String> {
    let Value::Rational(values) = &field(exif, Tag::ExposureTime)?.value else {
        return None;
    };
    let value = *values.first()?;
    let seconds = rational_value(value)?;
    if seconds > 0.0 && seconds < 1.0 {
        let reciprocal = (1.0 / seconds).round();
        Some(format!("{seconds:.3}s (1/{reciprocal:.0})"))
    } else {
        Some(format!("{seconds:.1} s"))
    }
}

fn exposure_program(value: u32) -> String {
    match value {
        0 => "Not defined",
        1 => "Manual",
        2 => "Program (auto)",
        3 => "Aperture priority",
        4 => "Shutter priority",
        5 => "Creative program",
        6 => "Action program",
        7 => "Portrait mode",
        8 => "Landscape mode",
        9 => "Bulb",
        _ => "Unknown",
    }
    .to_owned()
}

fn metering_mode(value: u32) -> String {
    match value {
        0 => "Unknown",
        1 => "Average",
        2 => "Center-weighted average",
        3 => "Spot",
        4 => "Multi-spot",
        5 => "Pattern",
        6 => "Partial",
        255 => "Other",
        _ => "Unknown",
    }
    .to_owned()
}

fn gps(exif: &Exif) -> Option<String> {
    let latitude = coordinate(exif, Tag::GPSLatitude)?;
    let longitude = coordinate(exif, Tag::GPSLongitude)?;
    let latitude_ref = ascii_field(exif, Tag::GPSLatitudeRef).unwrap_or_else(|| "N".to_owned());
    let longitude_ref = ascii_field(exif, Tag::GPSLongitudeRef).unwrap_or_else(|| "E".to_owned());
    let mut value = format!(
        "{} {}, {} {}",
        latitude_ref,
        format_coordinate(latitude),
        longitude_ref,
        format_coordinate(longitude)
    );
    if let Some(altitude) = gps_altitude(exif) {
        value.push_str(&format!(", {altitude:.1}m"));
    }
    Some(value)
}

fn coordinate(exif: &Exif, tag: Tag) -> Option<[f64; 3]> {
    let Value::Rational(values) = &field(exif, tag)?.value else {
        return None;
    };
    Some([
        rational_value(*values.first()?)?,
        rational_value(*values.get(1)?)?,
        rational_value(*values.get(2)?)?,
    ])
}

fn format_coordinate(value: [f64; 3]) -> String {
    format!("{:.0}°{:.0}'{:.2}\"", value[0], value[1], value[2])
}

fn gps_altitude(exif: &Exif) -> Option<f64> {
    let mut altitude = rational_field(exif, Tag::GPSAltitude)?;
    if unsigned_field(exif, Tag::GPSAltitudeRef) == Some(1) {
        altitude = -altitude;
    }
    Some(altitude)
}

fn color_space(exif: &Exif) -> Option<String> {
    match unsigned_field(exif, Tag::ColorSpace)? {
        1 => Some("sRGB".to_owned()),
        0xffff => Some("Uncalibrated".to_owned()),
        _ => None,
    }
}

fn read_jpeg_chroma_subsampling(path: &Path) -> Option<String> {
    let mut reader = BufReader::new(File::open(path).ok()?);
    let mut signature = [0_u8; 2];
    reader.read_exact(&mut signature).ok()?;
    if signature != [0xff, 0xd8] {
        return None;
    }

    loop {
        let mut byte = [0_u8; 1];
        reader.read_exact(&mut byte).ok()?;
        if byte[0] != 0xff {
            continue;
        }
        while reader.read_exact(&mut byte).is_ok() && byte[0] == 0xff {}
        let marker = byte[0];
        if marker == 0xda || marker == 0xd9 {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }

        let mut length = [0_u8; 2];
        reader.read_exact(&mut length).ok()?;
        let payload_len = usize::from(u16::from_be_bytes(length)).checked_sub(2)?;
        if is_start_of_frame(marker) {
            if payload_len > 512 {
                return None;
            }
            let mut payload = vec![0_u8; payload_len];
            reader.read_exact(&mut payload).ok()?;
            return chroma_subsampling_from_sof(&payload).map(str::to_owned);
        }
        reader
            .seek(SeekFrom::Current(i64::try_from(payload_len).ok()?))
            .ok()?;
    }
}

fn is_start_of_frame(marker: u8) -> bool {
    (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc)
}

fn chroma_subsampling_from_sof(payload: &[u8]) -> Option<&'static str> {
    let component_count = usize::from(*payload.get(5)?);
    if component_count != 3 || payload.len() < 6 + component_count * 3 {
        return None;
    }
    let y_sampling = *payload.get(7)?;
    let cb_sampling = *payload.get(10)?;
    let cr_sampling = *payload.get(13)?;
    if cb_sampling != 0x11 || cr_sampling != 0x11 {
        return None;
    }
    match (y_sampling >> 4, y_sampling & 0x0f) {
        (1, 1) => Some("YUV444"),
        (2, 1) => Some("YUV422"),
        (2, 2) => Some("YUV420"),
        (1, 2) => Some("YUV440"),
        (4, 1) => Some("YUV411"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_exif_datetime_and_coordinates() {
        assert_eq!(
            format_exif_datetime("2023:06:09 10:23:54".to_owned()),
            "2023/06/09 10:23:54"
        );
        assert_eq!(format_coordinate([36.0, 44.0, 2.62]), "36°44'2.62\"");
    }

    #[test]
    fn reads_jpeg_chroma_subsampling() {
        let yuv420 = [8, 0, 16, 0, 16, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1];
        assert_eq!(chroma_subsampling_from_sof(&yuv420), Some("YUV420"));
    }

    #[test]
    fn maps_exposure_and_metering_modes() {
        assert_eq!(exposure_program(2), "Program (auto)");
        assert_eq!(metering_mode(1), "Average");
    }
}
