use crate::{
    APPLICATION_ID,
    application::{
        config::{ConfigStore, LibraryLocator},
        controller::{
            ApplicationState, Command, ErrorKind, LibraryController, OpenError, OpenInfo,
            validate_folder_name,
        },
    },
    i18n::gettext,
};
use adw::prelude::*;
use std::{rc::Rc, sync::mpsc::Sender, time::Duration};

struct WindowUi {
    window: adw::ApplicationWindow,
    stack: gtk::Stack,
    commands: Sender<Command>,
}

pub(crate) fn build(application: &adw::Application) {
    if let Some(window) = application.windows().first() {
        window.present();
        return;
    }
    let store = ConfigStore::new(
        &gtk::glib::user_config_dir(),
        &gtk::glib::user_data_dir(),
        APPLICATION_ID,
    );
    let (commands, states) = LibraryController::start(store);
    let stack = gtk::Stack::builder().hexpand(true).vexpand(true).build();
    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("Pigoune")
        .default_width(900)
        .default_height(620)
        .content(&stack)
        .build();
    let ui = Rc::new(WindowUi {
        window,
        stack,
        commands,
    });
    ui.show(ApplicationState::Opening);
    let receiver_ui = ui.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(30), move || {
        while let Ok(state) = states.try_recv() {
            receiver_ui.show(state);
        }
        gtk::glib::ControlFlow::Continue
    });
    ui.send(Command::Startup);
    ui.window.present();
}

impl WindowUi {
    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            eprintln!("Library worker stopped unexpectedly");
        }
    }

    fn show(self: &Rc<Self>, state: ApplicationState) {
        let (name, view) = match state {
            ApplicationState::Welcome => ("welcome", self.welcome()),
            ApplicationState::Opening => ("opening", self.opening()),
            ApplicationState::Open(info) => ("open", self.open(info)),
            ApplicationState::OpenError(error) => ("error", self.error(error)),
        };
        if let Some(child) = self.stack.child_by_name(name) {
            self.stack.remove(&child);
        }
        self.stack.add_named(&view, Some(name));
        self.stack.set_visible_child_name(name);
    }

    fn page(&self, content: &impl IsA<gtk::Widget>) -> adw::ToolbarView {
        let view = adw::ToolbarView::builder().content(content).build();
        view.add_top_bar(&adw::HeaderBar::new());
        view
    }

    fn action(label: &str) -> gtk::Button {
        gtk::Button::builder()
            .label(label)
            .halign(gtk::Align::Center)
            .build()
    }

    fn buttons(buttons: &[gtk::Button]) -> gtk::Box {
        let box_ = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();
        for button in buttons {
            box_.append(button);
        }
        box_
    }

    fn welcome(self: &Rc<Self>) -> adw::ToolbarView {
        let create = Self::action(&gettext("Create Library"));
        create.add_css_class("suggested-action");
        let ui = self.clone();
        create.connect_clicked(move |_| {
            ui.show(ApplicationState::Opening);
            ui.send(Command::CreateManagedDefault);
        });
        let elsewhere = Self::action(&gettext("Create Elsewhere…"));
        let ui = self.clone();
        elsewhere.connect_clicked(move |_| ui.select_folder(true));
        let open = Self::action(&gettext("Open Library…"));
        let ui = self.clone();
        open.connect_clicked(move |_| ui.select_folder(false));
        let status = adw::StatusPage::builder()
            .icon_name("folder-pictures-symbolic")
            .title(gettext("Welcome to Pigoune"))
            .description(gettext("Create a library or open an existing one."))
            .child(&Self::buttons(&[create, elsewhere, open]))
            .build();
        self.page(&status)
    }

    fn opening(&self) -> adw::ToolbarView {
        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .halign(gtk::Align::Center)
            .build();
        let status = adw::StatusPage::builder()
            .title(gettext("Opening library…"))
            .child(&spinner)
            .build();
        self.page(&status)
    }

    fn open(self: &Rc<Self>, info: OpenInfo) -> adw::ToolbarView {
        let location_kind = match info.locator {
            LibraryLocator::ManagedDefault => "managed",
            LibraryLocator::FileUri { .. } => "external",
        };
        eprintln!("Opened library {} ({location_kind})", info.id);
        let title = if info.display_name.is_empty() {
            gettext("Library")
        } else {
            info.display_name
        };
        let status = adw::StatusPage::builder()
            .icon_name("image-x-generic-symbolic")
            .title(gettext("Library is open"))
            .build();
        let box_ = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .halign(gtk::Align::Center)
            .build();
        if let Some(diagnostic) = info.persistence_warning {
            eprintln!("Configuration persistence failed: {diagnostic}");
            let warning = gtk::Label::builder()
                .label(gettext(
                    "The library is open, but Pigoune could not confirm that its location was saved.",
                ))
                .wrap(true)
                .build();
            box_.append(&warning);
            let retry = Self::action(&gettext("Retry saving location"));
            let ui = self.clone();
            retry.connect_clicked(move |_| ui.send(Command::RetryPersistConfiguration));
            box_.append(&retry);
        }
        let open = Self::action(&gettext("Open Library…"));
        let ui = self.clone();
        open.connect_clicked(move |_| ui.select_folder(false));
        box_.append(&open);
        let create = Self::action(&gettext("Create Elsewhere…"));
        let ui = self.clone();
        create.connect_clicked(move |_| ui.select_folder(true));
        box_.append(&create);
        status.set_child(Some(&box_));
        let header = adw::HeaderBar::builder()
            .title_widget(&adw::WindowTitle::new("Pigoune", &title))
            .build();
        let view = adw::ToolbarView::builder().content(&status).build();
        view.add_top_bar(&header);
        view
    }

    fn error(self: &Rc<Self>, error: OpenError) -> adw::ToolbarView {
        eprintln!("Library error: {}", error.diagnostic);
        let forget_durability_uncertain =
            matches!(error.kind, ErrorKind::ForgetDurabilityUncertain);
        let (title, description) = match error.kind {
            ErrorKind::LocationUnavailable => (
                gettext("Library location unavailable"),
                gettext("The library location cannot be accessed."),
            ),
            ErrorKind::StorageUnavailable => (
                gettext("Library storage unavailable"),
                gettext("This location cannot complete the file operations required by a library."),
            ),
            ErrorKind::NotALibrary => (
                gettext("Not a Pigoune library"),
                gettext("The selected folder does not contain a complete Pigoune library."),
            ),
            ErrorKind::InvalidLibrary => (
                gettext("Invalid library"),
                gettext("The library contains invalid data and could not be opened."),
            ),
            ErrorKind::IncompatibleVersion => (
                gettext("Incompatible library version"),
                gettext("This version of Pigoune cannot open this library."),
            ),
            ErrorKind::InvalidConfiguration => (
                gettext("Invalid application configuration"),
                gettext("The saved library location is invalid."),
            ),
            ErrorKind::PersistenceFailed => (
                gettext("Could not update configuration"),
                gettext("The library location could not be forgotten."),
            ),
            ErrorKind::ForgetDurabilityUncertain => (
                gettext("Could not confirm the configuration change"),
                gettext(
                    "The library location was removed, but the change could not be confirmed on disk. Retry before closing Pigoune.",
                ),
            ),
            ErrorKind::DestinationExists => (
                gettext("Library already exists"),
                gettext("The destination already contains a folder. It was not replaced."),
            ),
        };
        let other = Self::action(&gettext("Open Another Library…"));
        let ui = self.clone();
        other.connect_clicked(move |_| ui.select_folder(false));
        let mut buttons = Vec::new();
        if error.has_session {
            let back = Self::action(&gettext("Back to Library"));
            let ui = self.clone();
            back.connect_clicked(move |_| ui.send(Command::ReturnToOpen));
            buttons.push(back);
        }
        if error.can_retry && !error.can_open_managed {
            let retry = Self::action(&gettext("Retry"));
            let ui = self.clone();
            retry.connect_clicked(move |_| {
                ui.show(ApplicationState::Opening);
                ui.send(Command::RetryOpen);
            });
            buttons.push(retry);
        }
        buttons.push(other);
        if error.can_open_managed {
            let existing = Self::action(&gettext("Open Existing Library"));
            let ui = self.clone();
            existing.connect_clicked(move |_| {
                ui.show(ApplicationState::Opening);
                ui.send(Command::OpenManagedDefault);
            });
            buttons.push(existing);
        }
        if error.configured {
            let label = if forget_durability_uncertain {
                gettext("Retry saving change")
            } else {
                gettext("Forget this library location")
            };
            let forget = Self::action(&label);
            let ui = self.clone();
            forget.connect_clicked(move |_| ui.send(Command::ForgetConfigured));
            buttons.push(forget);
        }
        let status = adw::StatusPage::builder()
            .icon_name("dialog-warning-symbolic")
            .title(title)
            .description(description)
            .child(&Self::buttons(&buttons))
            .build();
        self.page(&status)
    }

    fn select_folder(self: &Rc<Self>, create: bool) {
        let title = if create {
            gettext("Choose a parent folder")
        } else {
            gettext("Open Library")
        };
        let dialog = gtk::FileDialog::builder().title(title).build();
        let ui = self.clone();
        dialog.select_folder(
            Some(&self.window),
            None::<&gio::Cancellable>,
            move |result| match result {
                Ok(file) if create => ui.ask_folder_name(file),
                Ok(file) => match LibraryLocator::from_file(&file) {
                    Ok((locator, path)) => {
                        ui.show(ApplicationState::Opening);
                        ui.send(Command::OpenExternal { locator, path });
                    }
                    Err(error) => ui.selection_error(&error.to_string()),
                },
                Err(error) if error.matches(gtk::DialogError::Dismissed) => {}
                Err(error) => ui.selection_error(&error.to_string()),
            },
        );
    }

    fn ask_folder_name(self: &Rc<Self>, parent: gio::File) {
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("New library folder"))
            .build();
        dialog.add_response("cancel", &gettext("Cancel"));
        dialog.add_response("create", &gettext("Create Library"));
        dialog.set_default_response(Some("create"));
        let entry = gtk::Entry::builder()
            .placeholder_text(gettext("Folder name"))
            .activates_default(true)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(18)
            .margin_end(18)
            .build();
        dialog.set_extra_child(Some(&entry));
        let ui = self.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "create" {
                let name = entry.text().to_string();
                if !validate_folder_name(&name) {
                    ui.selection_error(&gettext("Enter a single folder name without a slash."));
                } else {
                    let child = parent.child(&name);
                    match LibraryLocator::from_file(&child) {
                        Ok((locator, path)) => {
                            ui.show(ApplicationState::Opening);
                            ui.send(Command::CreateExternal { locator, path });
                        }
                        Err(error) => ui.selection_error(&error.to_string()),
                    }
                }
            }
        });
        dialog.present(Some(&self.window));
    }

    fn selection_error(&self, diagnostic: &str) {
        eprintln!("Selected location error: {diagnostic}");
        let dialog = adw::AlertDialog::builder()
            .heading(gettext("Location unavailable"))
            .body(gettext("Choose a local folder that Pigoune can access."))
            .build();
        dialog.add_response("close", &gettext("Close"));
        dialog.present(Some(&self.window));
    }
}
