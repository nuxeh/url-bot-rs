use crate::{
    config::{Conf, ConfSet},
    cli::url_bot_rs::ExportFormat
};

pub fn export(configs: &[Conf], format: ExportFormat) -> String {
    let mut set = ConfSet::new();

    configs.iter()
        .for_each(|c| {
            set.configs.insert(c.network.name.clone(), c.clone());
            ()
        });

    String::from("foo")
}

#[cfg(test)]
mod tests {
    use super::*;

}
