//! Linux desktop appearance comes from the settings portal, not window decorations.
use ashpd::desktop::settings::{ColorScheme, Settings};
use futures_util::StreamExt;

use crate::render::Theme;

fn theme(scheme: ColorScheme) -> Theme {
	match scheme {
		ColorScheme::PreferDark => Theme::Dark,
		ColorScheme::PreferLight | ColorScheme::NoPreference => Theme::Light,
	}
}

pub(crate) async fn watch(changed: impl Fn(Theme)) -> ashpd::Result<()> {
	let settings = Settings::new().await?;
	// Subscribe before reading so a change during startup is not lost.
	let mut changes = settings.receive_color_scheme_changed().await?;
	let scheme = settings.color_scheme().await?;
	changed(theme(scheme));
	while let Some(scheme) = changes.next().await {
		changed(theme(scheme));
	}
	Ok(())
}
