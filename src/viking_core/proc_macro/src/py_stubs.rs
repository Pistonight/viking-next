use pm::pre::*;

/// Parse the input as an item and strip every `#[pyo3(...)]` attribute
/// from it and from the places inside it where pyo3 accepts them
/// (fields, enum variants, impl items and function arguments)
pub fn remove_pyo3(input: TokenStream) -> pm::Result<TokenStream2> {
    let mut item: syn::Item = syn::parse(input)?;
    match &mut item {
        syn::Item::Struct(x) => {
            strip(&mut x.attrs);
            strip_fields(&mut x.fields);
        }
        syn::Item::Enum(x) => {
            strip(&mut x.attrs);
            for variant in &mut x.variants {
                strip(&mut variant.attrs);
                strip_fields(&mut variant.fields);
            }
        }
        syn::Item::Union(x) => {
            strip(&mut x.attrs);
            for field in &mut x.fields.named {
                strip(&mut field.attrs);
            }
        }
        syn::Item::Fn(x) => {
            strip(&mut x.attrs);
            strip_sig(&mut x.sig);
        }
        syn::Item::Impl(x) => {
            strip(&mut x.attrs);
            for impl_item in &mut x.items {
                match impl_item {
                    syn::ImplItem::Fn(f) => {
                        strip(&mut f.attrs);
                        strip_sig(&mut f.sig);
                    }
                    syn::ImplItem::Const(c) => strip(&mut c.attrs),
                    _ => {}
                }
            }
        }
        other => pm::bail!(other, "unsupported item for pyo3 stub macro"),
    }
    Ok(item.into_token_stream())
}

fn strip(attrs: &mut Vec<syn::Attribute>) {
    const PYO3_ATTRS: &[&str] = &[
        "pyo3",
        "new",
        "getter",
        "setter",
        "deleter",
        "staticmethod",
        "classmethod",
        "classattr",
        "args",
    ];
    attrs.retain(|attr| !PYO3_ATTRS.iter().any(|name| attr.path().is_ident(name)));
}

fn strip_fields(fields: &mut syn::Fields) {
    for field in fields.iter_mut() {
        strip(&mut field.attrs);
    }
}

fn strip_sig(sig: &mut syn::Signature) {
    for input in &mut sig.inputs {
        match input {
            syn::FnArg::Receiver(r) => strip(&mut r.attrs),
            syn::FnArg::Typed(t) => strip(&mut t.attrs),
        }
    }
}
