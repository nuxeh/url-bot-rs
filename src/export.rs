use crate::{
    config::{Conf, ConfSet},
    cli::url_bot_rs::ExportFormat
};

pub fn export(configs: &[Conf], format: ExportFormat) -> String {
    let set = ConfSet::new();

    String::from("foo")
}
