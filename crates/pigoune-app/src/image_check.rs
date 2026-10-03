use std::path::Path;

use gtk::{gio, glib};

const SAMPLE_PNG: [u8; 70] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60, 0x60, 0x60, 0x60,
    0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0xA5, 0xF6, 0x45, 0x40, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

pub struct ImageCheck {
    context: glib::MainContext,
    decoder_works: bool,
}

impl ImageCheck {
    pub fn start() -> Self {
        let context = glib::MainContext::new();
        let decoder_works = context
            .block_on(decode(glycin::Loader::new_vec(SAMPLE_PNG.to_vec())))
            .is_ok();
        if !decoder_works {
            glib::g_warning!(
                "pigoune",
                "Image decoding is unavailable; imported images are only checked by their header"
            );
        }
        Self {
            context,
            decoder_works,
        }
    }

    pub fn is_intact(&self, path: &Path) -> bool {
        if !self.decoder_works {
            return true;
        }
        let mut loader = glycin::Loader::new(gio::File::for_path(path));
        loader.use_expose_base_dir(false);
        match self.context.block_on(decode(loader)) {
            Ok(()) => true,
            Err(error) => error.is_out_of_memory() || error.is_timeout(),
        }
    }
}

async fn decode(loader: glycin::Loader) -> Result<(), glycin::Error> {
    let mut image = loader.load().await?;
    image.next_frame().await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::ImageCheck;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    #[test]
    fn complete_images_are_intact() {
        let check = ImageCheck::start();
        for name in [
            "dark-circle.svg",
            "red-dot.png",
            "blue-photo.jpg",
            "spinner.gif",
        ] {
            assert!(check.is_intact(&fixture(name)), "{name}");
        }
    }

    #[test]
    fn an_image_cut_in_half_is_not_intact() {
        let bytes = fs::read(fixture("blue-photo.jpg")).expect("fixture exists");
        let cut = std::env::temp_dir().join(format!("pigoune-cut-{}.jpg", std::process::id()));
        fs::write(&cut, &bytes[..bytes.len() / 2]).expect("cut image written");

        let intact = ImageCheck::start().is_intact(&cut);

        fs::remove_file(&cut).expect("cut image removed");
        assert!(!intact);
    }
}
