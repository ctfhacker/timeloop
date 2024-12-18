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

// Function to extract the type name as a String
fn get_impl_name(ty: &Type) -> Option<String> {
    if let Type::Path(TypePath { path, .. }) = ty {
        // Extract the last segment of the path (the actual type name)
        if let Some(segment) = path.segments.last() {
            // Handle generic types by ignoring the generic arguments
            let ident = &segment.ident;
            Some(ident.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

#[proc_macro_attribute]
pub fn profile_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut impl_block = parse_macro_input!(item as ItemImpl);

    let type_name = get_impl_name(&impl_block.self_ty).unwrap_or_else(|| "UnknownType".to_string());

    for item in &mut impl_block.items {
        if let ImplItem::Fn(ref mut func) = item {
            let func_name = func.sig.ident.to_string();
            let identifier = format!("{type_name}::{func_name}");

            let scoped_timer = syn::parse_quote! {
                timeloop::scoped_timer!(#identifier);
            };

            func.block.stmts.insert(0, scoped_timer);
        }
    }

    let new_impl = quote! {
        #impl_block
    };

    TokenStream::from(new_impl)
}
