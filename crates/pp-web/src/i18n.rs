use dioxus_i18n::prelude::*;
use dioxus_i18n::unic_langid::langid;

pub fn init_i18n() {
    use_init_i18n(|| {
        I18nConfig::new(langid!("zh-CN"))
            .with_locale(Locale::new_static(
                langid!("zh-CN"),
                include_str!("../locales/zh-CN.ftl"),
            ))
            .with_locale(Locale::new_static(
                langid!("en-US"),
                include_str!("../locales/en-US.ftl"),
            ))
    });
}
