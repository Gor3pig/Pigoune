use gettextrs::gettext;

use crate::update_news::UpdateNews;

pub struct BannerWording {
    pub title: String,
    pub button: Option<String>,
}

#[must_use]
pub fn wording(news: &UpdateNews) -> BannerWording {
    match news {
        UpdateNews::Available => BannerWording {
            title: gettext("A new version of Pigoune is available"),
            button: Some(gettext("_Update")),
        },
        UpdateNews::Installing(percent) => BannerWording {
            title: gettext("Installing the update… {percent}%")
                .replace("{percent}", &percent.to_string()),
            button: None,
        },
        UpdateNews::Installed => BannerWording {
            title: gettext("The update is installed: restart Pigoune to use it"),
            button: Some(gettext("_Restart")),
        },
        UpdateNews::Failed => BannerWording {
            title: gettext("The update could not be installed, try again from Software"),
            button: None,
        },
    }
}

#[must_use]
pub fn restart_by_hand() -> BannerWording {
    BannerWording {
        title: gettext("Close and reopen Pigoune to use the new version"),
        button: None,
    }
}
