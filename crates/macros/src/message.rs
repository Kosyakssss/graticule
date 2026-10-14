use proc_macro2::TokenStream;
use quote::quote;

pub fn id_of(text: &str) -> String {
    let mut id = String::new();
    for word in text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).take(6) {
        if !id.is_empty() {
            id.push('-');
        }
        id.extend(word.chars().flat_map(char::to_lowercase));
    }
    if id.is_empty() {
        id.push_str("text");
    }
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("{id}-{:04x}", hash & 0xffff)
}

pub fn has_args(text: &str) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
            }
            '{' => return true,
            _ => {}
        }
    }
    false
}

pub fn expand(lit: syn::LitStr) -> syn::Result<TokenStream> {
    let kernel = crate::kernel();
    let text = lit.value();
    let id = id_of(&text);
    Ok(if has_args(&text) { quote!(#kernel::Message::formatted(#id, #lit, ::std::format!(#lit))) } else { quote!(#kernel::Message::fixed(#id, #lit)) })
}
