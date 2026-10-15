use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(lit: syn::LitStr) -> syn::Result<TokenStream> {
    let kernel = crate::kernel();
    let text = lit.value();
    let id = messages::id_of(&text);
    if !messages::has_args(&text) {
        return Ok(quote!(#kernel::Message::fixed(#id, #lit)));
    }
    let values = messages::names(&text).into_iter().map(|n| {
        let ident = quote::format_ident!("{}", n);
        quote!((#n, (&#kernel::language::Wrap(&#ident)).arg()))
    });
    Ok(quote! {
        #kernel::Message::formatted(#id, #lit, ::std::format!(#lit), || {
            use #kernel::language::Argument as _;
            ::std::vec![#(#values),*]
        })
    })
}
