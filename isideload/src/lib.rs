use rootcause::{
    hooks::{Hooks, context_formatter::ContextFormatterHook},
    prelude::*,
};

pub mod anisette;
pub mod auth;
pub mod util;

struct ReqwestErrorFormatter;

impl ContextFormatterHook<reqwest::Error> for ReqwestErrorFormatter {
    fn display(
        &self,
        report: rootcause::ReportRef<'_, reqwest::Error, markers::Uncloneable, markers::Local>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(f, "{}", report.format_current_context_unhooked())?;
        let mut source = report.current_context_error_source();
        while let Some(s) = source {
            writeln!(f, "Caused by: {:?}", s)?;
            source = s.source();
        }
        Ok(())
    }
}

pub fn init() -> Result<(), Report> {
    Hooks::new()
        .context_formatter::<reqwest::Error, _>(ReqwestErrorFormatter)
        .install()
        .context("Failed to install error reporting hooks")?;
    Ok(())
}
