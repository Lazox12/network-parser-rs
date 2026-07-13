mod utils;

#[allow(dead_code)] //todo
use proc_macro::TokenStream;
use std::str::FromStr;
use proc_macro2::Ident;
use proc_macro_warning::Warning;
use quote::quote;
use syn::{Expr, ExprClosure};
use crate::Row::Field;
/*
syntax:


make_struct! { MyStruct,
    field1: u3,
    consume(5) //skips 5 bytes
    field2: u5,
    field3: Vec<u8>;field1, // vec of size defined in field1
    field4: CStr,
    if peek(u8)==15{
        field5: u16,
        //field 5 will be defined as Option<u16> filled only when the next two bytes are 15
    }
    field6: [u8;4],
}
*/

enum Type{
    UInt(u8),
    Int(u8),
    Vec(Option<Ident>), //identifier of the size field
    CStr(Option<Ident>),
    Slice(u8),
    Optional(OptionType), // field defined behind if condition
}
struct OptionType{
    pub ty: Box<Vec<Type>>, // vec is here because multiple types can be defined behind the if condition
    pub condition: TokenStream
}

enum Row {
    Consume(Expr),
    Field(Field),

}
struct Field{
    identifier:Ident,
    ty:Type,
}
struct SyntaxTree{
    pub inner:Vec<Row>,
    pub test:String
}
impl From<TokenStream> for SyntaxTree{
    fn from(value: TokenStream) -> Self {
        let val_str = value.to_string();

        let rows = val_str.split(",");
        let mut tree:SyntaxTree;

        for row in rows{
            let row = row.trim();
            match row {
                s if s.starts_with("consume(")  => {
                    let ind = utils::find_closing_parrent('(',String::from(s));

                    tree.inner.push(Row::Consume(syn::parse_str(&s[8..ind]).unwrap()))

                }
                _ => {}
            }
        }

        SyntaxTree{inner:Vec::new(),test:val_str}

    }
}
#[proc_macro]
pub fn make_struct(input: TokenStream) -> TokenStream {
    let parsed: SyntaxTree = input.into();
    let test_content = parsed.test;

    let expanded = quote! {
        println!("{}", #test_content);
    };

    TokenStream::from(expanded)
}