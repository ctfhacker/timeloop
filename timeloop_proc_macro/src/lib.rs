use proc_macro::TokenStream;
use quote::quote;
use syn::*;

#[proc_macro_attribute]
pub fn profile(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut func = parse_macro_input!(item as ItemFn);

    let func_name = func.sig.ident.to_string();
    let identifier = format!("Fn__{func_name}");

    let scoped_timer = syn::parse_quote! {
        timeloop::scoped_timer!(#identifier);
    };

    func.block.stmts.insert(0, scoped_timer);

    let new_func = quote! {
        #func
    };

    TokenStream::from(new_func)
}
