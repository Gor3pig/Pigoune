use gettextrs::pgettext;
use pigoune_core::CollectionLook;

pub const DEFAULT_ICON_KEY: &str = "folder";
pub const DEFAULT_ICON_NAME: &str = "folder-symbolic";
pub const PHOSPHOR_ICON_PIXELS: i32 = 18;

pub struct LookChoice {
    pub key: &'static str,
    pub label: String,
}

macro_rules! icon_groups {
    ($($theme:literal => [$($key:literal),* $(,)?]),* $(,)?) => {
        const ICON_GROUPS: &[(&str, &[&str])] = &[$(($theme, &[$($key),*])),*];
        const ICON_KEYS: &[&str] = &[$($($key),*),*];
        const ICON_NAMES: &[&str] = &[$($(concat!($key, "-symbolic")),*),*];
    };
}

icon_groups![
    "files" => [
        "file-text",
        "table",
        "presentation-chart",
        "text-aa",
        "package",
        "paperclip",
        "clipboard",
        "notebook",
        "newspaper",
        "book",
        "books",
        "book-open",
        "bookmark-simple",
        "calendar-blank",
        "layout",
        "textbox",
    ],
    "art" => [
        "paint-brush-broad",
        "paint-bucket",
        "palette",
        "pencil-simple",
        "pen-nib",
        "eyedropper",
        "scissors",
        "ruler",
        "shapes",
        "magic-wand",
        "sparkle",
        "paint-brush",
        "paint-brush-household",
        "paint-roller",
        "highlighter",
        "eraser",
        "pencil-ruler",
        "pen",
        "bezier-curve",
        "vector-three",
        "path",
        "polygon",
        "hexagon",
        "circle-half",
        "cube",
        "cylinder",
        "three-d",
        "gradient",
        "swatches",
        "crop",
        "frame-corners",
        "perspective",
        "signature",
        "scribble",
        "spiral",
        "stamp",
        "sticker",
        "blueprint",
        "lasso",
        "vignette",
        "spray-bottle",
    ],
    "photo" => [
        "image",
        "images",
        "film-strip",
        "film-slate",
        "aperture",
        "camera",
        "video",
        "film-reel",
        "film-script",
        "panorama",
        "television",
    ],
    "music" => [
        "music-notes",
        "guitar",
        "vinyl-record",
        "headphones",
        "piano-keys",
        "metronome",
        "microphone-stage",
        "cassette-tape",
        "disco-ball",
    ],
    "nature" => [
        "tree",
        "tree-evergreen",
        "tree-palm",
        "leaf",
        "flower",
        "flower-lotus",
        "plant",
        "cactus",
        "mountains",
        "cloud",
        "cloud-rain",
        "snowflake",
        "lightning",
        "flame",
        "drop",
        "waves",
        "rainbow",
        "moon-stars",
        "sun",
        "moon",
        "acorn",
        "flower-tulip",
        "potted-plant",
        "clover",
        "feather",
        "park",
        "island",
        "sun-horizon",
        "sun-dim",
        "cloud-sun",
        "cloud-moon",
        "cloud-snow",
        "cloud-fog",
        "cloud-lightning",
        "rainbow-cloud",
        "thermometer",
        "wind",
        "tornado",
        "hurricane",
        "meteor",
        "shooting-star",
        "planet",
        "star-four",
        "drop-half",
        "campfire",
        "windmill",
        "solar-panel",
    ],
    "animals" => [
        "cat",
        "dog",
        "bird",
        "fish",
        "butterfly",
        "bug",
        "horse",
        "cow",
        "paw-print",
        "rabbit",
        "bug-beetle",
        "fish-simple",
        "egg",
        "shrimp",
        "bone",
    ],
    "places" => [
        "house",
        "buildings",
        "storefront",
        "castle-turret",
        "church",
        "mosque",
        "hospital",
        "factory",
        "city",
        "bridge",
        "lighthouse",
        "garage",
        "warehouse",
        "crane-tower",
        "hard-hat",
        "door-open",
        "map-pin",
        "map-trifold",
        "globe",
        "globe-stand",
        "globe-hemisphere-west",
    ],
    "travel" => [
        "car",
        "bicycle",
        "train",
        "rocket-launch",
        "sailboat",
        "tent",
        "backpack",
        "suitcase",
        "lifebuoy",
        "airplane",
        "airplane-tilt",
        "bus",
        "taxi",
        "tram",
        "subway",
        "cable-car",
        "motorcycle",
        "scooter",
        "truck",
        "van",
        "jeep",
        "police-car",
        "ambulance",
        "fire-truck",
        "boat",
        "parachute",
        "steering-wheel",
        "gas-pump",
        "traffic-signal",
        "signpost",
        "compass-rose",
        "suitcase-rolling",
        "ticket",
        "compass",
        "anchor",
        "road-horizon",
    ],
    "home" => [
        "armchair",
        "couch",
        "lamp",
        "bed",
        "bathtub",
        "swimming-pool",
        "lightbulb",
        "flashlight",
        "dress",
        "pants",
        "hoodie",
        "high-heel",
        "boot",
        "sunglasses",
        "t-shirt",
        "sneaker",
        "umbrella",
        "handbag",
        "shopping-cart",
    ],
    "food" => [
        "coffee",
        "pizza",
        "cake",
        "hamburger",
        "wine",
        "champagne",
        "beer-stein",
        "ice-cream",
        "popcorn",
        "fork-knife",
        "chef-hat",
    ],
    "games" => [
        "game-controller",
        "dice-five",
        "puzzle-piece",
        "trophy",
        "medal",
        "target",
        "crown",
        "diamond",
        "balloon",
        "gift",
        "confetti",
        "joystick",
        "lego",
        "cards-three",
        "poker-chip",
        "basketball",
        "soccer-ball",
        "boxing-glove",
        "barbell",
        "flag-checkered",
        "treasure-chest",
        "mask-happy",
        "sword",
    ],
    "science" => [
        "atom",
        "flask",
        "microscope",
        "stethoscope",
        "first-aid-kit",
        "heartbeat",
        "tooth",
        "brain",
        "code",
        "terminal-window",
        "robot",
        "alien",
        "laptop",
        "monitor",
        "desktop",
        "device-mobile",
        "phone-handset",
        "device-tablet",
        "wrench",
        "hammer",
        "toolbox",
        "screwdriver",
        "magnet",
        "calculator",
        "chart-bar",
        "graduation-cap",
        "briefcase",
        "binoculars",
    ],
    "symbols" => [
        "star",
        "heart",
        "smiley",
        "hand-waving",
        "user-circle",
        "users-three",
        "infinity",
        "ghost",
        "skull",
        "clock",
        "hourglass",
        "watch",
        "scales",
        "lock",
        "key",
        "shield",
        "flag",
    ],
];

const LEGACY_ICON_KEYS: &[(&str, &str)] = &[
    ("folder-pictures", "images"),
    ("folder-music", "music-notes"),
    ("folder-videos", "video"),
    ("folder-templates", "layout"),
    ("user-home", "house"),
    ("user-bookmarks", "bookmark-simple"),
    ("starred", "star"),
    ("emote-love", "heart"),
    ("camera-photo", "camera"),
    ("audio-headphones", "headphones"),
    ("font-x-generic", "text-aa"),
    ("x-office-document", "file-text"),
    ("x-office-drawing", "pencil-simple"),
    ("x-office-presentation", "presentation-chart"),
    ("x-office-spreadsheet", "table"),
    ("x-office-calendar", "calendar-blank"),
    ("mail-attachment", "paperclip"),
    ("package-x-generic", "package"),
    ("applications-graphics", "palette"),
    ("applications-games", "game-controller"),
    ("applications-science", "flask"),
    ("applications-engineering", "wrench"),
    ("dialog-password", "key"),
    ("emoji-nature", "leaf"),
    ("emoji-food", "fork-knife"),
    ("emoji-travel", "suitcase"),
    ("airplane-mode", "airplane"),
    ("emoji-objects", "lightbulb"),
    ("emoji-symbols", "infinity"),
    ("emoji-flags", "flag"),
    ("emoji-activities", "soccer-ball"),
    ("emoji-people", "users-three"),
    ("face-smile", "smiley"),
    ("weather-clear", "sun"),
    ("weather-clear-night", "moon"),
    ("computer", "desktop"),
    ("phone", "phone-handset"),
    ("input-tablet", "device-tablet"),
    ("mark-location", "map-pin"),
];

const COLOR_KEYS: [&str; 9] = [
    "blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "slate",
];

pub const COLOR_CLASSES: [&str; 9] = [
    "collection-blue",
    "collection-teal",
    "collection-green",
    "collection-yellow",
    "collection-orange",
    "collection-red",
    "collection-pink",
    "collection-purple",
    "collection-slate",
];

#[must_use]
pub fn icon_name(look: &CollectionLook) -> &'static str {
    icon_name_of_key(known_icon_key(look))
}

#[must_use]
pub fn icon_name_of_key(key: &str) -> &'static str {
    ICON_KEYS
        .iter()
        .position(|known| *known == key)
        .map_or(DEFAULT_ICON_NAME, |index| ICON_NAMES[index])
}

#[must_use]
pub fn is_phosphor_icon(name: &str) -> bool {
    ICON_NAMES.contains(&name)
}

#[must_use]
pub fn known_icon_key(look: &CollectionLook) -> &'static str {
    look.icon
        .as_deref()
        .and_then(current_icon_key)
        .unwrap_or(DEFAULT_ICON_KEY)
}

fn current_icon_key(key: &str) -> Option<&'static str> {
    let current = LEGACY_ICON_KEYS
        .iter()
        .find(|(old, _)| *old == key)
        .map_or(key, |(_, new)| *new);
    ICON_KEYS.iter().find(|known| **known == current).copied()
}

#[must_use]
pub fn known_color_key(look: &CollectionLook) -> Option<&'static str> {
    let key = look.color.as_deref()?;
    COLOR_KEYS.iter().find(|known| **known == key).copied()
}

#[must_use]
pub fn color_class(look: &CollectionLook) -> Option<&'static str> {
    let key = known_color_key(look)?;
    COLOR_KEYS
        .iter()
        .position(|known| *known == key)
        .map(|index| COLOR_CLASSES[index])
}

pub struct IconGroup {
    pub title: String,
    pub choices: Vec<LookChoice>,
}

#[must_use]
pub fn icon_groups() -> Vec<IconGroup> {
    ICON_GROUPS
        .iter()
        .map(|(theme, keys)| IconGroup {
            title: theme_title(theme),
            choices: keys
                .iter()
                .map(|key| LookChoice {
                    key,
                    label: icon_label(key),
                })
                .collect(),
        })
        .collect()
}

#[must_use]
pub fn default_icon_label() -> String {
    icon_label(DEFAULT_ICON_KEY)
}

fn theme_title(theme: &str) -> String {
    match theme {
        "files" => pgettext("collection theme", "Files and Documents"),
        "art" => pgettext("collection theme", "Art and Design"),
        "photo" => pgettext("collection theme", "Photo and Video"),
        "music" => pgettext("collection theme", "Music and Audio"),
        "nature" => pgettext("collection theme", "Nature and Weather"),
        "animals" => pgettext("collection theme", "Animals"),
        "places" => pgettext("collection theme", "Places and Buildings"),
        "travel" => pgettext("collection theme", "Travel and Transport"),
        "home" => pgettext("collection theme", "Home and Clothing"),
        "food" => pgettext("collection theme", "Food and Drink"),
        "games" => pgettext("collection theme", "Games and Sports"),
        "science" => pgettext("collection theme", "Science, Tech and Work"),
        _ => pgettext("collection theme", "Symbols and People"),
    }
}

#[must_use]
pub fn search_text(label: &str, key: &str) -> String {
    format!("{}\n{}", folded(label), folded(&key.replace('-', " ")))
}

#[must_use]
pub fn search_wanted(query: &str) -> String {
    folded(query.trim())
}

#[must_use]
pub fn matches_wanted(search_text: &str, wanted: &str) -> bool {
    wanted.is_empty() || search_text.contains(wanted)
}

fn folded(text: &str) -> String {
    text.to_lowercase().chars().map(plain_letters).collect()
}

fn plain_letters(letter: char) -> String {
    match letter {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ą' => "a".to_owned(),
        'ç' | 'ć' => "c".to_owned(),
        'è' | 'é' | 'ê' | 'ë' | 'ę' => "e".to_owned(),
        'ì' | 'í' | 'î' | 'ï' => "i".to_owned(),
        'ł' => "l".to_owned(),
        'ñ' | 'ń' => "n".to_owned(),
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => "o".to_owned(),
        'ś' => "s".to_owned(),
        'ù' | 'ú' | 'û' | 'ü' => "u".to_owned(),
        'ý' | 'ÿ' => "y".to_owned(),
        'ź' | 'ż' => "z".to_owned(),
        'æ' => "ae".to_owned(),
        'œ' => "oe".to_owned(),
        'ß' => "ss".to_owned(),
        other => other.to_string(),
    }
}

#[must_use]
pub fn color_choices() -> Vec<LookChoice> {
    COLOR_KEYS
        .iter()
        .map(|key| LookChoice {
            key,
            label: color_label(key),
        })
        .collect()
}

#[must_use]
pub fn default_color_label() -> String {
    pgettext("collection color", "Default")
}

fn icon_label(key: &str) -> String {
    icon_label_first(key)
        .or_else(|| icon_label_second(key))
        .or_else(|| icon_label_third(key))
        .or_else(|| icon_label_fourth(key))
        .or_else(|| icon_label_fifth(key))
        .or_else(|| icon_label_sixth(key))
        .unwrap_or_else(|| pgettext("collection icon", "Folder"))
}

fn icon_label_first(key: &str) -> Option<String> {
    let label = match key {
        "file-text" => pgettext("collection icon", "Document"),
        "table" => pgettext("collection icon", "Spreadsheet"),
        "presentation-chart" => pgettext("collection icon", "Presentation"),
        "text-aa" => pgettext("collection icon", "Font"),
        "package" => pgettext("collection icon", "Package"),
        "paperclip" => pgettext("collection icon", "Paper Clip"),
        "clipboard" => pgettext("collection icon", "Clipboard"),
        "notebook" => pgettext("collection icon", "Notebook"),
        "newspaper" => pgettext("collection icon", "Newspaper"),
        "book" => pgettext("collection icon", "Book"),
        "books" => pgettext("collection icon", "Books"),
        "book-open" => pgettext("collection icon", "Open Book"),
        "bookmark-simple" => pgettext("collection icon", "Page Marker"),
        "calendar-blank" => pgettext("collection icon", "Blank Calendar"),
        "layout" => pgettext("collection icon", "Templates"),
        "textbox" => pgettext("collection icon", "Text Box"),
        "paint-brush-broad" => pgettext("collection icon", "Wide Brush"),
        "paint-bucket" => pgettext("collection icon", "Paint Bucket"),
        "palette" => pgettext("collection icon", "Palette"),
        "pencil-simple" => pgettext("collection icon", "Pencil"),
        "pen-nib" => pgettext("collection icon", "Pen Nib"),
        "eyedropper" => pgettext("collection icon", "Eyedropper"),
        "scissors" => pgettext("collection icon", "Scissors"),
        "ruler" => pgettext("collection icon", "Ruler"),
        "shapes" => pgettext("collection icon", "Shapes"),
        "magic-wand" => pgettext("collection icon", "Magic Wand"),
        "sparkle" => pgettext("collection icon", "Sparkle"),
        "paint-brush" => pgettext("collection icon", "Paint Brush"),
        "paint-brush-household" => pgettext("collection icon", "Household Brush"),
        "paint-roller" => pgettext("collection icon", "Paint Roller"),
        "highlighter" => pgettext("collection icon", "Highlighter"),
        "eraser" => pgettext("collection icon", "Eraser"),
        "pencil-ruler" => pgettext("collection icon", "Pencil and Ruler"),
        "pen" => pgettext("collection icon", "Pen"),
        "bezier-curve" => pgettext("collection icon", "Bézier Curve"),
        "vector-three" => pgettext("collection icon", "Vector Shape"),
        "path" => pgettext("collection icon", "Path"),
        "polygon" => pgettext("collection icon", "Polygon"),
        "hexagon" => pgettext("collection icon", "Hexagon"),
        "circle-half" => pgettext("collection icon", "Half Circle"),
        "cube" => pgettext("collection icon", "Cube"),
        "cylinder" => pgettext("collection icon", "Cylinder"),
        "three-d" => pgettext("collection icon", "3D"),
        "gradient" => pgettext("collection icon", "Gradient"),
        "swatches" => pgettext("collection icon", "Color Swatches"),
        "crop" => pgettext("collection icon", "Crop"),
        "frame-corners" => pgettext("collection icon", "Frame"),
        "perspective" => pgettext("collection icon", "Perspective"),
        "signature" => pgettext("collection icon", "Signature"),
        "scribble" => pgettext("collection icon", "Scribble"),
        "spiral" => pgettext("collection icon", "Spiral"),
        "stamp" => pgettext("collection icon", "Stamp"),
        "sticker" => pgettext("collection icon", "Sticker"),
        "blueprint" => pgettext("collection icon", "Blueprint"),
        "lasso" => pgettext("collection icon", "Lasso"),
        _ => return None,
    };
    Some(label)
}

fn icon_label_second(key: &str) -> Option<String> {
    let label = match key {
        "vignette" => pgettext("collection icon", "Vignette"),
        "spray-bottle" => pgettext("collection icon", "Spray Bottle"),
        "image" => pgettext("collection icon", "Image"),
        "images" => pgettext("collection icon", "Photos"),
        "film-strip" => pgettext("collection icon", "Film Strip"),
        "film-slate" => pgettext("collection icon", "Clapperboard"),
        "aperture" => pgettext("collection icon", "Aperture"),
        "camera" => pgettext("collection icon", "Camera"),
        "video" => pgettext("collection icon", "Videos"),
        "film-reel" => pgettext("collection icon", "Film Reel"),
        "film-script" => pgettext("collection icon", "Screenplay"),
        "panorama" => pgettext("collection icon", "Panorama"),
        "television" => pgettext("collection icon", "Television"),
        "music-notes" => pgettext("collection icon", "Music Notes"),
        "guitar" => pgettext("collection icon", "Guitar"),
        "vinyl-record" => pgettext("collection icon", "Vinyl Record"),
        "headphones" => pgettext("collection icon", "Headphones"),
        "piano-keys" => pgettext("collection icon", "Piano Keys"),
        "metronome" => pgettext("collection icon", "Metronome"),
        "microphone-stage" => pgettext("collection icon", "Stage Microphone"),
        "cassette-tape" => pgettext("collection icon", "Cassette Tape"),
        "disco-ball" => pgettext("collection icon", "Disco Ball"),
        "tree" => pgettext("collection icon", "Tree"),
        "tree-evergreen" => pgettext("collection icon", "Evergreen Tree"),
        "tree-palm" => pgettext("collection icon", "Palm Tree"),
        "leaf" => pgettext("collection icon", "Leaf"),
        "flower" => pgettext("collection icon", "Flower"),
        "flower-lotus" => pgettext("collection icon", "Lotus"),
        "plant" => pgettext("collection icon", "Plant"),
        "cactus" => pgettext("collection icon", "Cactus"),
        "mountains" => pgettext("collection icon", "Mountains"),
        "cloud" => pgettext("collection icon", "Cloud"),
        "cloud-rain" => pgettext("collection icon", "Rain"),
        "snowflake" => pgettext("collection icon", "Snowflake"),
        "lightning" => pgettext("collection icon", "Lightning"),
        "flame" => pgettext("collection icon", "Flame"),
        "drop" => pgettext("collection icon", "Water Drop"),
        "waves" => pgettext("collection icon", "Waves"),
        "rainbow" => pgettext("collection icon", "Rainbow"),
        "moon-stars" => pgettext("collection icon", "Moon and Stars"),
        "sun" => pgettext("collection icon", "Sun"),
        "moon" => pgettext("collection icon", "Moon"),
        "acorn" => pgettext("collection icon", "Acorn"),
        "flower-tulip" => pgettext("collection icon", "Tulip"),
        "potted-plant" => pgettext("collection icon", "Potted Plant"),
        "clover" => pgettext("collection icon", "Clover"),
        "feather" => pgettext("collection icon", "Feather"),
        "park" => pgettext("collection icon", "Park"),
        "island" => pgettext("collection icon", "Island"),
        "sun-horizon" => pgettext("collection icon", "Sunrise"),
        "sun-dim" => pgettext("collection icon", "Dim Light"),
        "cloud-sun" => pgettext("collection icon", "Partly Sunny"),
        "cloud-moon" => pgettext("collection icon", "Cloudy Night"),
        "cloud-snow" => pgettext("collection icon", "Snow"),
        "cloud-fog" => pgettext("collection icon", "Fog"),
        _ => return None,
    };
    Some(label)
}

fn icon_label_third(key: &str) -> Option<String> {
    let label = match key {
        "cloud-lightning" => pgettext("collection icon", "Thunderstorm"),
        "rainbow-cloud" => pgettext("collection icon", "Rainbow Cloud"),
        "thermometer" => pgettext("collection icon", "Thermometer"),
        "wind" => pgettext("collection icon", "Wind"),
        "tornado" => pgettext("collection icon", "Tornado"),
        "hurricane" => pgettext("collection icon", "Hurricane"),
        "meteor" => pgettext("collection icon", "Meteor"),
        "shooting-star" => pgettext("collection icon", "Shooting Star"),
        "planet" => pgettext("collection icon", "Planet"),
        "star-four" => pgettext("collection icon", "Four-Point Star"),
        "drop-half" => pgettext("collection icon", "Half Drop"),
        "campfire" => pgettext("collection icon", "Campfire"),
        "windmill" => pgettext("collection icon", "Windmill"),
        "solar-panel" => pgettext("collection icon", "Solar Panel"),
        "cat" => pgettext("collection icon", "Cat"),
        "dog" => pgettext("collection icon", "Dog"),
        "bird" => pgettext("collection icon", "Bird"),
        "fish" => pgettext("collection icon", "Fish"),
        "butterfly" => pgettext("collection icon", "Butterfly"),
        "bug" => pgettext("collection icon", "Bug"),
        "horse" => pgettext("collection icon", "Horse"),
        "cow" => pgettext("collection icon", "Cow"),
        "paw-print" => pgettext("collection icon", "Paw Print"),
        "rabbit" => pgettext("collection icon", "Rabbit"),
        "bug-beetle" => pgettext("collection icon", "Beetle"),
        "fish-simple" => pgettext("collection icon", "Small Fish"),
        "egg" => pgettext("collection icon", "Egg"),
        "shrimp" => pgettext("collection icon", "Shrimp"),
        "bone" => pgettext("collection icon", "Bone"),
        "house" => pgettext("collection icon", "House"),
        "buildings" => pgettext("collection icon", "Buildings"),
        "storefront" => pgettext("collection icon", "Shop"),
        "castle-turret" => pgettext("collection icon", "Castle"),
        "church" => pgettext("collection icon", "Church"),
        "mosque" => pgettext("collection icon", "Mosque"),
        "hospital" => pgettext("collection icon", "Hospital"),
        "factory" => pgettext("collection icon", "Factory"),
        "city" => pgettext("collection icon", "City"),
        "bridge" => pgettext("collection icon", "Bridge"),
        "lighthouse" => pgettext("collection icon", "Lighthouse"),
        "garage" => pgettext("collection icon", "Garage"),
        "warehouse" => pgettext("collection icon", "Warehouse"),
        "crane-tower" => pgettext("collection icon", "Tower Crane"),
        "hard-hat" => pgettext("collection icon", "Hard Hat"),
        "door-open" => pgettext("collection icon", "Open Door"),
        "map-pin" => pgettext("collection icon", "Map Pin"),
        "map-trifold" => pgettext("collection icon", "Map"),
        "globe" => pgettext("collection icon", "Globe"),
        "globe-stand" => pgettext("collection icon", "Desk Globe"),
        "globe-hemisphere-west" => pgettext("collection icon", "Americas"),
        "car" => pgettext("collection icon", "Car"),
        "bicycle" => pgettext("collection icon", "Bicycle"),
        "train" => pgettext("collection icon", "Train"),
        "rocket-launch" => pgettext("collection icon", "Rocket"),
        "sailboat" => pgettext("collection icon", "Sailboat"),
        _ => return None,
    };
    Some(label)
}

fn icon_label_fourth(key: &str) -> Option<String> {
    let label = match key {
        "tent" => pgettext("collection icon", "Tent"),
        "backpack" => pgettext("collection icon", "Backpack"),
        "suitcase" => pgettext("collection icon", "Suitcase"),
        "lifebuoy" => pgettext("collection icon", "Lifebuoy"),
        "airplane" => pgettext("collection icon", "Airplane"),
        "airplane-tilt" => pgettext("collection icon", "Airplane Taking Off"),
        "bus" => pgettext("collection icon", "Bus"),
        "taxi" => pgettext("collection icon", "Taxi"),
        "tram" => pgettext("collection icon", "Tram"),
        "subway" => pgettext("collection icon", "Subway"),
        "cable-car" => pgettext("collection icon", "Cable Car"),
        "motorcycle" => pgettext("collection icon", "Motorcycle"),
        "scooter" => pgettext("collection icon", "Scooter"),
        "truck" => pgettext("collection icon", "Truck"),
        "van" => pgettext("collection icon", "Van"),
        "jeep" => pgettext("collection icon", "Jeep"),
        "police-car" => pgettext("collection icon", "Police Car"),
        "ambulance" => pgettext("collection icon", "Ambulance"),
        "fire-truck" => pgettext("collection icon", "Fire Truck"),
        "boat" => pgettext("collection icon", "Boat"),
        "parachute" => pgettext("collection icon", "Parachute"),
        "steering-wheel" => pgettext("collection icon", "Steering Wheel"),
        "gas-pump" => pgettext("collection icon", "Gas Pump"),
        "traffic-signal" => pgettext("collection icon", "Traffic Light"),
        "signpost" => pgettext("collection icon", "Signpost"),
        "compass-rose" => pgettext("collection icon", "Compass Rose"),
        "suitcase-rolling" => pgettext("collection icon", "Rolling Suitcase"),
        "ticket" => pgettext("collection icon", "Ticket"),
        "compass" => pgettext("collection icon", "Compass"),
        "anchor" => pgettext("collection icon", "Anchor"),
        "road-horizon" => pgettext("collection icon", "Road"),
        "armchair" => pgettext("collection icon", "Armchair"),
        "couch" => pgettext("collection icon", "Couch"),
        "lamp" => pgettext("collection icon", "Lamp"),
        "bed" => pgettext("collection icon", "Bed"),
        "bathtub" => pgettext("collection icon", "Bathtub"),
        "swimming-pool" => pgettext("collection icon", "Swimming Pool"),
        "lightbulb" => pgettext("collection icon", "Light Bulb"),
        "flashlight" => pgettext("collection icon", "Flashlight"),
        "dress" => pgettext("collection icon", "Dress"),
        "pants" => pgettext("collection icon", "Pants"),
        "hoodie" => pgettext("collection icon", "Hoodie"),
        "high-heel" => pgettext("collection icon", "High Heel"),
        "boot" => pgettext("collection icon", "Boot"),
        "sunglasses" => pgettext("collection icon", "Sunglasses"),
        "t-shirt" => pgettext("collection icon", "T-Shirt"),
        "sneaker" => pgettext("collection icon", "Sneaker"),
        "umbrella" => pgettext("collection icon", "Umbrella"),
        "handbag" => pgettext("collection icon", "Handbag"),
        "shopping-cart" => pgettext("collection icon", "Shopping Cart"),
        "coffee" => pgettext("collection icon", "Coffee"),
        "pizza" => pgettext("collection icon", "Pizza"),
        "cake" => pgettext("collection icon", "Cake"),
        "hamburger" => pgettext("collection icon", "Hamburger"),
        "wine" => pgettext("collection icon", "Wine"),
        _ => return None,
    };
    Some(label)
}

fn icon_label_fifth(key: &str) -> Option<String> {
    let label = match key {
        "champagne" => pgettext("collection icon", "Champagne"),
        "beer-stein" => pgettext("collection icon", "Beer Mug"),
        "ice-cream" => pgettext("collection icon", "Ice Cream"),
        "popcorn" => pgettext("collection icon", "Popcorn"),
        "fork-knife" => pgettext("collection icon", "Cutlery"),
        "chef-hat" => pgettext("collection icon", "Chef Hat"),
        "game-controller" => pgettext("collection icon", "Game Controller"),
        "dice-five" => pgettext("collection icon", "Dice"),
        "puzzle-piece" => pgettext("collection icon", "Puzzle Piece"),
        "trophy" => pgettext("collection icon", "Trophy"),
        "medal" => pgettext("collection icon", "Medal"),
        "target" => pgettext("collection icon", "Target"),
        "crown" => pgettext("collection icon", "Crown"),
        "diamond" => pgettext("collection icon", "Diamond"),
        "balloon" => pgettext("collection icon", "Balloon"),
        "gift" => pgettext("collection icon", "Gift"),
        "confetti" => pgettext("collection icon", "Confetti"),
        "joystick" => pgettext("collection icon", "Joystick"),
        "lego" => pgettext("collection icon", "Building Blocks"),
        "cards-three" => pgettext("collection icon", "Playing Cards"),
        "poker-chip" => pgettext("collection icon", "Poker Chip"),
        "basketball" => pgettext("collection icon", "Basketball"),
        "soccer-ball" => pgettext("collection icon", "Soccer Ball"),
        "boxing-glove" => pgettext("collection icon", "Boxing Glove"),
        "barbell" => pgettext("collection icon", "Barbell"),
        "flag-checkered" => pgettext("collection icon", "Checkered Flag"),
        "treasure-chest" => pgettext("collection icon", "Treasure Chest"),
        "mask-happy" => pgettext("collection icon", "Theater Mask"),
        "sword" => pgettext("collection icon", "Sword"),
        "atom" => pgettext("collection icon", "Atom"),
        "flask" => pgettext("collection icon", "Flask"),
        "microscope" => pgettext("collection icon", "Microscope"),
        "stethoscope" => pgettext("collection icon", "Stethoscope"),
        "first-aid-kit" => pgettext("collection icon", "First Aid Kit"),
        "heartbeat" => pgettext("collection icon", "Heartbeat"),
        "tooth" => pgettext("collection icon", "Tooth"),
        "brain" => pgettext("collection icon", "Brain"),
        "code" => pgettext("collection icon", "Code"),
        "terminal-window" => pgettext("collection icon", "Terminal"),
        "robot" => pgettext("collection icon", "Robot"),
        "alien" => pgettext("collection icon", "Alien"),
        "laptop" => pgettext("collection icon", "Laptop"),
        "monitor" => pgettext("collection icon", "Monitor"),
        "desktop" => pgettext("collection icon", "Computer"),
        "device-mobile" => pgettext("collection icon", "Mobile Device"),
        "phone-handset" => pgettext("collection icon", "Phone"),
        "device-tablet" => pgettext("collection icon", "Tablet"),
        "wrench" => pgettext("collection icon", "Wrench"),
        "hammer" => pgettext("collection icon", "Hammer"),
        "toolbox" => pgettext("collection icon", "Toolbox"),
        "screwdriver" => pgettext("collection icon", "Screwdriver"),
        "magnet" => pgettext("collection icon", "Magnet"),
        "calculator" => pgettext("collection icon", "Calculator"),
        "chart-bar" => pgettext("collection icon", "Bar Chart"),
        "graduation-cap" => pgettext("collection icon", "Graduation Cap"),
        _ => return None,
    };
    Some(label)
}

fn icon_label_sixth(key: &str) -> Option<String> {
    let label = match key {
        "briefcase" => pgettext("collection icon", "Briefcase"),
        "binoculars" => pgettext("collection icon", "Binoculars"),
        "star" => pgettext("collection icon", "Star"),
        "heart" => pgettext("collection icon", "Heart"),
        "smiley" => pgettext("collection icon", "Smile"),
        "hand-waving" => pgettext("collection icon", "Waving Hand"),
        "user-circle" => pgettext("collection icon", "User"),
        "users-three" => pgettext("collection icon", "Group"),
        "infinity" => pgettext("collection icon", "Infinity"),
        "ghost" => pgettext("collection icon", "Ghost"),
        "skull" => pgettext("collection icon", "Skull"),
        "clock" => pgettext("collection icon", "Clock"),
        "hourglass" => pgettext("collection icon", "Hourglass"),
        "watch" => pgettext("collection icon", "Wristwatch"),
        "scales" => pgettext("collection icon", "Scales"),
        "lock" => pgettext("collection icon", "Lock"),
        "key" => pgettext("collection icon", "Key"),
        "shield" => pgettext("collection icon", "Shield"),
        "flag" => pgettext("collection icon", "Pole Flag"),
        _ => return None,
    };
    Some(label)
}

fn color_label(key: &str) -> String {
    match key {
        "teal" => pgettext("collection color", "Teal"),
        "green" => pgettext("collection color", "Green"),
        "yellow" => pgettext("collection color", "Yellow"),
        "orange" => pgettext("collection color", "Orange"),
        "red" => pgettext("collection color", "Red"),
        "pink" => pgettext("collection color", "Pink"),
        "purple" => pgettext("collection color", "Purple"),
        "slate" => pgettext("collection color", "Slate"),
        _ => pgettext("collection color", "Blue"),
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::CollectionLook;

    use super::{
        COLOR_KEYS, DEFAULT_ICON_NAME, ICON_GROUPS, ICON_KEYS, LEGACY_ICON_KEYS, color_class,
        default_icon_label, icon_groups, icon_name, is_phosphor_icon, known_icon_key,
        matches_wanted, search_text, search_wanted,
    };

    #[test]
    fn every_icon_key_names_its_symbolic_icon() {
        for key in ICON_KEYS {
            let look = CollectionLook {
                icon: Some((*key).to_owned()),
                color: None,
            };
            assert_eq!(icon_name(&look), format!("{key}-symbolic"));
            assert!(look.is_valid());
        }
    }

    #[test]
    fn every_icon_belongs_to_one_non_empty_theme() {
        let in_groups: usize = ICON_GROUPS.iter().map(|(_, keys)| keys.len()).sum();
        assert_eq!(in_groups, ICON_KEYS.len());
        assert!(ICON_GROUPS.iter().all(|(_, keys)| !keys.is_empty()));
        assert_eq!(icon_groups().len(), ICON_GROUPS.len());
    }

    #[test]
    fn every_old_icon_key_still_shows_its_phosphor_equivalent() {
        for (old, new) in LEGACY_ICON_KEYS {
            let look = CollectionLook {
                icon: Some((*old).to_owned()),
                color: None,
            };
            assert!(ICON_KEYS.contains(new), "{new} is not a current icon");
            assert!(
                old == new || !ICON_KEYS.contains(old),
                "{old} must stay an alias only"
            );
            assert_eq!(known_icon_key(&look), *new);
            assert_eq!(icon_name(&look), format!("{new}-symbolic"));
        }
    }

    #[test]
    fn the_icons_of_the_first_versions_keep_their_meaning() {
        let starred = CollectionLook {
            icon: Some("starred".to_owned()),
            color: None,
        };
        let home = CollectionLook {
            icon: Some("user-home".to_owned()),
            color: None,
        };
        assert_eq!(icon_name(&starred), "star-symbolic");
        assert_eq!(icon_name(&home), "house-symbolic");
        assert_eq!(LEGACY_ICON_KEYS.len(), 39);
    }

    fn matches_search(label: &str, key: &str, query: &str) -> bool {
        matches_wanted(&search_text(label, key), &search_wanted(query))
    }

    #[test]
    fn searching_ignores_case_and_accents() {
        assert!(matches_search("Éclair", "lightning", "eclair"));
        assert!(matches_search("Chat", "cat", "CHA"));
        assert!(matches_search("Empreinte de patte", "paw-print", "de pat"));
        assert!(matches_search("Cœur", "heart", "coeur"));
        assert!(matches_search("Błyskawica", "lightning", "blyskawica"));
    }

    #[test]
    fn searching_also_reads_the_english_key() {
        assert!(matches_search("Empreinte de patte", "paw-print", "paw pri"));
        assert!(matches_search("Fusée", "rocket-launch", "rocket"));
    }

    #[test]
    fn an_empty_search_matches_everything_and_nonsense_nothing() {
        assert!(matches_search("Chat", "cat", "  "));
        assert!(!matches_search("Chat", "cat", "zzz"));
    }

    #[test]
    fn every_icon_has_its_file() {
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/scalable/actions");
        for key in ICON_KEYS {
            let file = folder.join(format!("{key}-symbolic.svg"));
            assert!(file.exists(), "{key} has no icon file");
        }
    }

    #[test]
    fn every_icon_file_is_declared_in_the_resources() {
        let resources = include_str!("../resources.gresource.xml");
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/scalable/actions");
        for entry in std::fs::read_dir(folder).expect("icons folder exists") {
            let name = entry.expect("icon entry is readable").file_name();
            let declaration = format!(
                "<file>icons/scalable/actions/{}</file>",
                name.to_string_lossy()
            );
            assert!(
                resources.contains(&declaration),
                "{declaration} is not embedded"
            );
        }
    }

    #[test]
    fn every_icon_key_has_an_embedded_file() {
        let resources = include_str!("../resources.gresource.xml");
        for key in ICON_KEYS {
            let declaration = format!("<file>icons/scalable/actions/{key}-symbolic.svg</file>");
            assert!(resources.contains(&declaration), "{key} is not embedded");
        }
    }

    #[test]
    fn only_the_collection_icons_are_phosphor_ones() {
        for key in ICON_KEYS {
            assert!(is_phosphor_icon(&format!("{key}-symbolic")));
        }
        assert!(!is_phosphor_icon(DEFAULT_ICON_NAME));
        assert!(!is_phosphor_icon("user-trash-symbolic"));
    }

    #[test]
    fn icon_keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for key in ICON_KEYS {
            assert!(seen.insert(*key), "{key} is listed twice");
        }
    }

    #[test]
    fn only_the_default_icon_is_labelled_folder() {
        assert_eq!(default_icon_label(), "Folder");
        for choice in icon_groups().iter().flat_map(|group| &group.choices) {
            assert_ne!(choice.label, "Folder", "{} is labelled Folder", choice.key);
        }
    }

    #[test]
    fn every_color_key_has_its_own_class() {
        for key in COLOR_KEYS {
            let look = CollectionLook {
                icon: None,
                color: Some(key.to_owned()),
            };
            assert_eq!(
                color_class(&look),
                Some(format!("collection-{key}").as_str())
            );
            assert!(look.is_valid());
        }
    }

    #[test]
    fn an_unknown_look_falls_back_to_the_default_folder() {
        let look = CollectionLook {
            icon: Some("rocket".to_owned()),
            color: Some("gold".to_owned()),
        };

        assert_eq!(icon_name(&look), DEFAULT_ICON_NAME);
        assert_eq!(color_class(&look), None);
        assert_eq!(icon_name(&CollectionLook::default()), DEFAULT_ICON_NAME);
    }
}
