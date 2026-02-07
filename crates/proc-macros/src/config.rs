use syn::parse::{Error, Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Expr, Ident, Meta, Path, Token};

#[derive(Debug)]
pub struct TestConfig {
    pub is_standalone: Option<Path>,
    pub is_cloneable: Option<Path>,
    pub tags: Vec<Ident>,
}

#[derive(Debug)]
pub enum Config {
    Params,
    Test(TestConfig),
}

impl Parse for Config {
    fn parse(input: ParseStream) -> Result<Self> {
        let list = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        let mut is_param_mode = false;
        let mut test_config = TestConfig {
            is_standalone: None,
            is_cloneable: None,
            tags: Vec::new(),
        };
        for param in list {
            if param.path().is_ident("cloneable") {
                test_config.is_cloneable = Some(param.path().clone());
            } else if param.path().is_ident("params") {
                is_param_mode = true;
            } else if param.path().is_ident("standalone") {
                test_config.is_standalone = Some(param.path().clone());
            } else if param.path().is_ident("tags") {
                let value = param.require_name_value()?;
                if let Expr::Array(list) = &value.value {
                    for tag in &list.elems {
                        if let Expr::Path(tag) = tag {
                            let Some(tag) = tag.path.get_ident() else {
                                return Err(Error::new(
                                    tag.span(),
                                    "Tags must be a simple ident, without path segments",
                                ));
                            };
                            test_config.tags.push(tag.clone());
                        } else {
                            return Err(Error::new(tag.span(), "Tags must be a simple idents"));
                        }
                    }
                } else {
                    return Err(Error::new(
                        value.span(),
                        "Tags must be an array e.g. [my_tag]",
                    ));
                }
            } else {
                return Err(Error::new(
                    param.span(),
                    "Unexpected name, possible values are: params, cloneable, standalone, tags",
                ));
            }
        }
        if is_param_mode {
            if let Some(path) = test_config.is_cloneable {
                return Err(Error::new(
                    path.span(),
                    "`cloneable` cannot be used in `params` mode",
                ));
            }
            if let Some(path) = test_config.is_standalone {
                return Err(Error::new(
                    path.span(),
                    "`standalone` cannot be used in `params` mode",
                ));
            }
            if let Some(path) = test_config.tags.first() {
                return Err(Error::new(
                    path.span(),
                    "`tags` cannot be used in `params` mode",
                ));
            }
            Ok(Config::Params)
        } else {
            Ok(Config::Test(test_config))
        }
    }
}
