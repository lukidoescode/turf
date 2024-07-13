//! You're probably looking for `turf` instead.

use convert_case::{Case, Casing};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use turf_internals::{CompiledStyleSheet, StyleSheetKind};

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro]
pub fn style_sheet(input: TokenStream) -> TokenStream {
    let input = input.to_string();
    let sanitized_path = PathBuf::from(input.trim_matches('"'));

    let ProcessedStyleSheet {
        untracked_load_paths,
        css,
        class_names,
    } = match handle_style_sheet(StyleSheetKind::File(sanitized_path)) {
        Ok(result) => result,
        Err(e) => {
            return match e {
                Error::Turf(e) => to_compile_error(e),
                Error::LoadPathTracking(e) => to_compile_error(e),
            }
        }
    };

    let mut out = quote! {
        pub static STYLE_SHEET: &'static str = #css;
    };
    out.extend(create_classes_structure(class_names));
    out.extend(create_include_bytes(untracked_load_paths));

    out.into()
}

#[proc_macro]
pub fn style_sheet_values(input: TokenStream) -> TokenStream {
    let input = input.to_string();
    let sanitized_path = PathBuf::from(input.trim_matches('"'));

    let ProcessedStyleSheet {
        untracked_load_paths,
        css,
        class_names,
    } = match handle_style_sheet(StyleSheetKind::File(sanitized_path)) {
        Ok(result) => result,
        Err(e) => {
            return match e {
                Error::Turf(e) => to_compile_error(e),
                Error::LoadPathTracking(e) => to_compile_error(e),
            }
        }
    };

    let includes = create_include_bytes(untracked_load_paths);
    let inlines = create_inline_classes_instance(class_names);
    let out = quote! {{
        pub static STYLE_SHEET: &'static str = #css;
        #includes
        #inlines
    }};

    out.into()
}

#[proc_macro]
pub fn inline_style_sheet(input: TokenStream) -> TokenStream {
    let input = input.to_string();

    let ProcessedStyleSheet {
        untracked_load_paths,
        css,
        class_names,
    } = match handle_style_sheet(StyleSheetKind::Inline(input)) {
        Ok(result) => result,
        Err(e) => {
            return match e {
                Error::Turf(e) => to_compile_error(e),
                Error::LoadPathTracking(e) => to_compile_error(e),
            }
        }
    };

    let mut out = quote! {
        pub static STYLE_SHEET: &'static str = #css;
    };
    out.extend(create_classes_structure(class_names));
    out.extend(create_include_bytes(untracked_load_paths));

    out.into()
}

#[proc_macro]
pub fn inline_style_sheet_values(input: TokenStream) -> TokenStream {
    let input = input.to_string();

    let ProcessedStyleSheet {
        untracked_load_paths,
        css,
        class_names,
    } = match handle_style_sheet(StyleSheetKind::Inline(input)) {
        Ok(result) => result,
        Err(e) => {
            return match e {
                Error::Turf(e) => to_compile_error(e),
                Error::LoadPathTracking(e) => to_compile_error(e),
            }
        }
    };

    let includes = create_include_bytes(untracked_load_paths);
    let inlines = create_inline_classes_instance(class_names);
    let out = quote! {{
        pub static STYLE_SHEET: &'static str = #css;
        #includes
        #inlines
    }};

    out.into()
}

fn to_compile_error<E>(e: E) -> TokenStream
where
    E: std::error::Error,
{
    let mut message = format!("Error: {}", e.to_string());
    let mut curr_err = e.source();

    if curr_err.is_some() {
        message.push_str(&format!("\nCaused by:"));
    }

    while let Some(current_error) = curr_err {
        message.push_str(&format!("\n    {}", current_error));
        curr_err = current_error.source();
    }

    quote! {
        compile_error!(#message);
    }
    .into()
}

fn create_classes_structure(classes: HashMap<String, String>) -> proc_macro2::TokenStream {
    let original_class_names: Vec<proc_macro2::Ident> = classes
        .keys()
        .map(|class| class.to_case(Case::ScreamingSnake))
        .map(|class| quote::format_ident!("{}", class.as_str().to_uppercase()))
        .collect();

    let randomized_class_names: Vec<&String> = classes.values().collect();

    let doc = original_class_names
        .iter()
        .zip(randomized_class_names.iter())
        .fold(String::new(), |mut doc, (variable, class_name)| {
            doc.push_str(&format!("{} = \"{}\"\n", variable, class_name));
            doc
        });

    quote::quote! {
        #[doc=#doc]
        pub struct ClassName;
        impl ClassName {
            #(pub const #original_class_names: &'static str = #randomized_class_names;)*
        }
    }
}

fn create_inline_classes_instance(classes: HashMap<String, String>) -> proc_macro2::TokenStream {
    let original_class_names: Vec<proc_macro2::Ident> = classes
        .keys()
        .map(|class| class.to_case(Case::Snake))
        .map(|class| quote::format_ident!("{}", class.as_str()))
        .collect();

    let randomized_class_names: Vec<&String> = classes.values().collect();

    let doc = original_class_names
        .iter()
        .zip(randomized_class_names.iter())
        .fold(String::new(), |mut doc, (variable, class_name)| {
            doc.push_str(&format!("{} = \"{}\"\n", variable, class_name));
            doc
        });

    quote::quote! {
        #[doc=#doc]
        pub struct ClassNames {
            #(pub #original_class_names: &'static str,)*
        }
        impl ClassNames {
            pub fn new() -> Self {
                Self {
                    #(#original_class_names: #randomized_class_names,)*
                }
            }
        }

        (STYLE_SHEET, ClassNames::new())
    }
}

fn create_include_bytes(untracked_load_paths: Vec<PathBuf>) -> proc_macro2::TokenStream {
    let untracked_load_path_values: Vec<String> = untracked_load_paths
        .into_iter()
        .map(|item| format!("{}", item.as_path().display()))
        .collect();

    quote::quote! {
        #(const _: &[u8] = include_bytes!(#untracked_load_path_values);)*
    }
}

enum Error {
    Turf(turf_internals::Error),
    LoadPathTracking(turf_internals::LoadPathTrackingError),
}

struct ProcessedStyleSheet {
    untracked_load_paths: Vec<PathBuf>,
    css: String,
    class_names: HashMap<String, String>,
}

fn handle_style_sheet(style_sheet: StyleSheetKind) -> Result<ProcessedStyleSheet, Error> {
    let CompiledStyleSheet {
        css,
        class_names,
        original_style_sheet,
    } = turf_internals::style_sheet(style_sheet).map_err(Error::Turf)?;

    let untracked_load_paths = {
        let mut values =
            turf_internals::get_untracked_load_paths().map_err(Error::LoadPathTracking)?;

        if let StyleSheetKind::File(current_file_path) = original_style_sheet {
            values.push(current_file_path);
        }

        values
    };

    Ok(ProcessedStyleSheet {
        untracked_load_paths,
        css,
        class_names,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::create_classes_structure;

    #[test]
    fn test() {
        let mut class_names = HashMap::new();
        class_names.insert(String::from("test-class"), String::from("abc-123"));

        let out = create_classes_structure(class_names);

        assert_eq!(
            out.to_string(),
            quote::quote! {
                #[doc="TEST_CLASS = \"abc-123\"\n"]
                pub struct ClassName;
                impl ClassName {
                    pub const TEST_CLASS: &'static str = "abc-123";
                }
            }
            .to_string()
        )
    }
}
