use lumia_core::GpsCoordinates;

use crate::app::LumiaApp;

impl LumiaApp {
    pub(crate) fn current_gps_coordinates(&self) -> Option<GpsCoordinates> {
        self.viewer
            .document()?
            .metadata
            .as_ref()?
            .exif
            .gps_coordinates
    }

    pub(crate) fn open_current_image_location(&self) {
        let Some(coordinates) = self.current_gps_coordinates() else {
            return;
        };
        let _ = crate::shell::open_url_in_browser(&open_street_map_url(coordinates));
    }
}

fn open_street_map_url(coordinates: GpsCoordinates) -> String {
    let latitude = coordinates.latitude_degrees();
    let longitude = coordinates.longitude_degrees();
    format!(
        "https://www.openstreetmap.org/?mlat={latitude:.7}&mlon={longitude:.7}#map=16/{latitude:.7}/{longitude:.7}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_url_marks_and_centers_the_gps_location() {
        let coordinates = GpsCoordinates::from_degrees(-33.856_784_4, 151.215_296_7).unwrap();
        assert_eq!(
            open_street_map_url(coordinates),
            "https://www.openstreetmap.org/?mlat=-33.8567844&mlon=151.2152967#map=16/-33.8567844/151.2152967"
        );
    }
}
