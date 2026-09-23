const APPLICATION_ID: &str = "io.github.Gor3pig.Pigoune";
const DEVELOPMENT_APPLICATION_ID: &str = "io.github.Gor3pig.Pigoune.Devel";

fn main() {
    let _ = (
        pigoune_core::CRATE_NAME,
        APPLICATION_ID,
        DEVELOPMENT_APPLICATION_ID,
    );
}
