#![no_std]
extern crate alloc;
use alloc::{vec,format};
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::string::{String, ToString};

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{Expr, LitInt, Ident};
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::ext::IdentExt;

enum Type{
    UInt(u8),
    Int(u8),
    Bool,
    USize,
    Vec(Option<syn::Expr>), // expression for the size in bytes
    CStr(()),
    Slice(u8),
    Optional(OptionType), // field defined behind if condition
    Box(Box<Type>),
    Exclude(Box<Type>),
    Custom(syn::Type),
}
struct OptionType{
    pub ty: Box<Vec<Type>>, // vec is here because multiple types can be defined behind the if condition
    pub condition: proc_macro2::TokenStream
}

enum ConsumeType{
    Literal(LitInt),
    Expr(Expr),
}

impl Parse for ConsumeType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(LitInt) {
            Ok(ConsumeType::Literal(input.parse()?))
        } else {
            Ok(ConsumeType::Expr(input.parse()?))
        }
    }
}

enum Row {
    Consume(ConsumeType),
    IfBlock {
        condition: proc_macro2::TokenStream,
        rows: Vec<Row>,
    },
    Field(Field),
}

struct Field{
    identifier: Ident,
    ty: Type,
}

struct SyntaxTree{
    pub attrs: Vec<syn::Attribute>,
    pub inner: Vec<Row>,
    pub struct_name: String
}
impl ToTokens for SyntaxTree {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let struct_name = Ident::new(&self.struct_name, proc_macro2::Span::call_site());
        
        let mut fields = Vec::new();
        for row in &self.inner {
            match row {
                Row::Field(f) => fields.push(quote!(#f)),
                Row::Consume(_) => {},
                Row::IfBlock { rows, .. } => {
                    for r in rows {
                        if let Row::Field(f) = r {
                            let ident = &f.identifier;
                            let ty = &f.ty;
                            fields.push(quote!(pub #ident: Option<#ty>));
                        }
                    }
                }
            }
        }

        let attrs = &self.attrs;

        tokens.extend(quote! {
            #(#attrs)*
            pub struct #struct_name {
                #(#fields,)*
            }
        });
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.identifier;
        let ty = &self.ty;
        tokens.extend(quote! {
            pub #ident: #ty
        });
    }
}

impl ToTokens for Type {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Type::Bool => { tokens.extend(quote!(bool)); }
            Type::USize => { tokens.extend(quote!(usize)); }
            Type::UInt(bits) | Type::Int(bits) => {
                let rust_ty = if *bits <= 8 { quote!(u8) }
                    else if *bits <= 16 { quote!(u16) }
                    else if *bits <= 32 { quote!(u32) }
                    else { quote!(u64) };

                if matches!(self, Type::Int(_)) {
                    let signed_ty = if *bits <= 8 { quote!(i8) }
                        else if *bits <= 16 { quote!(i16) }
                        else if *bits <= 32 { quote!(i32) }
                        else { quote!(i64) };
                    tokens.extend(signed_ty);
                } else {
                    tokens.extend(rust_ty);
                }
            }
            Type::Box(inner) => {
                let inner_tokens = quote!(#inner);
                tokens.extend(quote!(alloc::boxed::Box<#inner_tokens>));
            }
            Type::Exclude(inner) => {
                inner.to_tokens(tokens);
            }
            Type::Vec(_) => { tokens.extend(quote!(alloc::vec::Vec<u8>)); }
            Type::CStr(_) => { tokens.extend(quote!(alloc::ffi::CString)); }
            Type::Slice(len) => {
                let len_lit = proc_macro2::Literal::u8_unsuffixed(*len);
                tokens.extend(quote!([u8; #len_lit]));
            }
            Type::Optional(opt) => {
                let inner_ty = opt.ty.first().unwrap();
                tokens.extend(quote!(core::option::Option<#inner_ty>));
            }
            Type::Custom(ty) => {
                tokens.extend(quote!(#ty));
            }
        }
    }
}

impl SyntaxTree {
    fn generate_network_parse(&self) -> proc_macro2::TokenStream {
        let struct_name = Ident::new(&self.struct_name, proc_macro2::Span::call_site());
        
        let mut try_from_reads = Vec::new();
        let mut writes = Vec::new();
        let mut struct_fields = Vec::new();

        for row in &self.inner {
            match row {
                Row::Consume(c) => {
                    let amount = match c {
                        ConsumeType::Literal(lit) => quote!(#lit as usize),
                        ConsumeType::Expr(expr) => quote!((#expr) as usize),
                    };
                    let consume_stmt = quote! { bit_offset += #amount; };
                    try_from_reads.push(consume_stmt);
                    writes.push(quote! { write_bits(&mut buffer, &mut bit_offset, #amount, 0); });
                }
                Row::IfBlock { condition, rows } => {
                    let mut block_reads = Vec::new();
                    let mut block_writes = Vec::new();
                    let mut block_decls = Vec::new();
                    
                    let peek_ident = Ident::new("peek", proc_macro2::Span::call_site());
                    
                    let mut first_field_ident = None;
                    
                    for row in rows {
                        match row {
                            Row::Field(f) => {
                                let ident = &f.identifier;
                                let temp_ident = Ident::new(&format!("{}_temp", ident), proc_macro2::Span::call_site());
                                block_decls.push(quote! { let mut #ident = None; });
                                block_reads.push(f.ty.generate_parse_code(&temp_ident));
                                block_reads.push(quote! { #ident = Some(#temp_ident); });
                                
                                let inner_write = f.ty.generate_write_code(&quote!(*v));
                                block_writes.push(quote! {
                                    if let Some(v) = &(self.#ident) {
                                        #inner_write
                                    }
                                });
                                
                                struct_fields.push(ident);
                                
                                if first_field_ident.is_none() {
                                    first_field_ident = Some(ident.clone());
                                }
                            }
                            Row::Consume(c) => {
                                let amount = match c {
                                    ConsumeType::Literal(lit) => quote!(#lit as usize),
                                    ConsumeType::Expr(expr) => quote!((#expr) as usize),
                                };
                                block_reads.push(quote! { bit_offset += #amount; });
                                block_writes.push(quote! { write_bits(&mut buffer, &mut bit_offset, #amount, 0); });
                            }
                            _ => {}
                        }
                    }
                    
                    let condition_wrapper = quote! {
                        {
                            let #peek_ident = |bits: usize| -> u64 {
                                let mut temp_offset = bit_offset;
                                read_bits(data, &mut temp_offset, bits).unwrap_or(0)
                            };
                            #condition
                        }
                    };
                    
                    try_from_reads.push(quote! {
                        #(#block_decls)*
                        let condition_result = #condition_wrapper;
                        if condition_result {
                            #(#block_reads)*
                        }
                    });
                    
                    if let Some(first_ident) = first_field_ident {
                        writes.push(quote! {
                            if self.#first_ident.is_some() {
                                #(#block_writes)*
                            }
                        });
                    }
                }
                Row::Field(f) => {
                    let ident = &f.identifier;
                    let ty = &f.ty;
                    
                    try_from_reads.push(ty.generate_parse_code(ident));
                    writes.push(ty.generate_write_code(&quote!(self.#ident)));
                    
                    struct_fields.push(ident);
                }
            }
        }

        quote! {
            impl<'a> network_parser_rs::NetworkParse<'a> for #struct_name {
                fn parse_bits(data: &[u8], mut bit_offset_ref: &mut usize) -> core::result::Result<Self, &'static str> {
                    // Create a local alias for macro logic which uses `bit_offset`
                    let mut bit_offset = *bit_offset_ref;
                    
                    let read_bits = |data: &[u8], bit_offset: &mut usize, bits: usize| -> core::result::Result<u64, &'static str> {
                        if *bit_offset + bits > data.len() * 8 {
                            return Err("EOF");
                        }
                        if *bit_offset % 8 == 0 {
                            let byte_idx = *bit_offset / 8;
                            if bits == 8 {
                                *bit_offset += 8;
                                return Ok(data[byte_idx] as u64);
                            } else if bits == 16 {
                                *bit_offset += 16;
                                let mut buf = [0u8; 2];
                                buf.copy_from_slice(&data[byte_idx..byte_idx+2]);
                                return Ok(u16::from_be_bytes(buf) as u64);
                            } else if bits == 32 {
                                *bit_offset += 32;
                                let mut buf = [0u8; 4];
                                buf.copy_from_slice(&data[byte_idx..byte_idx+4]);
                                return Ok(u32::from_be_bytes(buf) as u64);
                            } else if bits == 64 {
                                *bit_offset += 64;
                                let mut buf = [0u8; 8];
                                buf.copy_from_slice(&data[byte_idx..byte_idx+8]);
                                return Ok(u64::from_be_bytes(buf) as u64);
                            }
                        }
                        
                        let mut val: u64 = 0;
                        for i in 0..bits {
                            let current_bit = *bit_offset + i;
                            let byte_idx = current_bit / 8;
                            let bit_idx = 7 - (current_bit % 8);
                            let bit = (data[byte_idx] >> bit_idx) & 1;
                            val = (val << 1) | (bit as u64);
                        }
                        *bit_offset += bits;
                        Ok(val)
                    };



                    #(#try_from_reads)*

                    *bit_offset_ref = bit_offset;
                    Ok(Self {
                        #(#struct_fields,)*
                    })
                }



                fn write_bits(&self, mut buffer: &mut alloc::vec::Vec<u8>, mut bit_offset_ref: &mut usize) {
                    let mut bit_offset = *bit_offset_ref;
                    
                    let mut write_bits = |buffer: &mut alloc::vec::Vec<u8>, bit_offset: &mut usize, bits: usize, val: u64| {
                        if *bit_offset % 8 == 0 {
                            let byte_idx = *bit_offset / 8;
                            if bits == 8 {
                                while buffer.len() <= byte_idx { buffer.push(0); }
                                buffer[byte_idx] = val as u8;
                                *bit_offset += 8;
                                return;
                            } else if bits == 16 {
                                while buffer.len() <= byte_idx + 1 { buffer.push(0); }
                                buffer[byte_idx..byte_idx+2].copy_from_slice(&(val as u16).to_be_bytes());
                                *bit_offset += 16;
                                return;
                            } else if bits == 32 {
                                while buffer.len() <= byte_idx + 3 { buffer.push(0); }
                                buffer[byte_idx..byte_idx+4].copy_from_slice(&(val as u32).to_be_bytes());
                                *bit_offset += 32;
                                return;
                            } else if bits == 64 {
                                while buffer.len() <= byte_idx + 7 { buffer.push(0); }
                                buffer[byte_idx..byte_idx+8].copy_from_slice(&val.to_be_bytes());
                                *bit_offset += 64;
                                return;
                            }
                        }
                        
                        for i in 0..bits {
                            let current_bit = *bit_offset + i;
                            let byte_idx = current_bit / 8;
                            let bit_idx = 7 - (current_bit % 8);
                            
                            while buffer.len() <= byte_idx {
                                buffer.push(0);
                            }
                            
                            let bit = (val >> (bits - 1 - i)) & 1;
                            buffer[byte_idx] |= (bit as u8) << bit_idx;
                        }
                        *bit_offset += bits;
                    };

                    #[allow(unused_variables)]


                    #(
                        let #struct_fields = self.#struct_fields.clone();
                    )*

                    #(#writes)*

                    *bit_offset_ref = bit_offset;
                }
            }
        }
    }
}

impl Type {
    fn generate_parse_code(&self, ident: &Ident) -> proc_macro2::TokenStream {
        match self {
            Type::UInt(bits) => {
                let bits_lit = proc_macro2::Literal::u8_unsuffixed(*bits);
                let rust_ty = if *bits <= 8 { quote!(u8) }
                    else if *bits <= 16 { quote!(u16) }
                    else if *bits <= 32 { quote!(u32) }
                    else { quote!(u64) };

                quote! {
                    let #ident = read_bits(&data, &mut bit_offset, #bits_lit as usize)? as #rust_ty;
                }
            }
            Type::Bool => {
                quote! {
                    let #ident = read_bits(&data, &mut bit_offset, 1)? == 1;
                }
            }
            Type::USize => {
                quote! {
                    let bits = core::mem::size_of::<usize>() * 8;
                    let #ident = read_bits(&data, &mut bit_offset, bits)? as usize;
                }
            }
            Type::Int(bits) => {
                let bits_lit = proc_macro2::Literal::u8_unsuffixed(*bits);
                let rust_ty = if *bits <= 8 { quote!(i8) }
                    else if *bits <= 16 { quote!(i16) }
                    else if *bits <= 32 { quote!(i32) }
                    else { quote!(i64) };

                quote! {
                    let raw = read_bits(&data, &mut bit_offset, #bits_lit as usize)?;
                    let sign_extend = if (raw & (1 << (#bits_lit - 1))) != 0 {
                        !0u64 << #bits_lit
                    } else {
                        0
                    };
                    let #ident = (raw | sign_extend) as #rust_ty;
                }
            }
            Type::Slice(len) => {
                let len_lit = proc_macro2::Literal::u8_unsuffixed(*len);
                quote! {
                    if bit_offset % 8 != 0 { return Err("Unaligned byte read for slice"); }
                    let byte_offset = bit_offset / 8;
                    if byte_offset + (#len_lit as usize) > data.len() { return Err("EOF"); }
                    let mut #ident = [0u8; #len_lit as usize];
                    #ident.copy_from_slice(&data[byte_offset .. byte_offset + (#len_lit as usize)]);
                    bit_offset += (#len_lit as usize) * 8;
                }
            }
            Type::Vec(size_opt) => {
                if let Some(size_expr) = size_opt {
                    quote! {
                        if bit_offset % 8 != 0 { return Err("Unaligned byte read for Vec"); }
                        let byte_offset = bit_offset / 8;
                        let len = (#size_expr) as usize;
                        if byte_offset + len > data.len() { return Err("EOF"); }
                        let #ident = data[byte_offset .. byte_offset + len].to_vec();
                        bit_offset += len * 8;
                    }
                } else {
                    quote! {
                        if bit_offset % 8 != 0 { return Err("Unaligned byte read for Vec"); }
                        let byte_offset = bit_offset / 8;
                        let #ident = data[byte_offset..].to_vec();
                        bit_offset = data.len() * 8;
                    }
                }
            }
            Type::CStr(_) => {
                quote! {
                    if bit_offset % 8 != 0 { return Err("Unaligned byte read for CStr"); }
                    let byte_offset = bit_offset / 8;
                    let mut end = byte_offset;
                    while end < data.len() && data[end] != 0 {
                        end += 1;
                    }
                    if end >= data.len() { return Err("EOF before null terminator"); }
                    let #ident = CString::new(data[byte_offset..end].to_vec())
                        .map_err(|_| "Invalid CStr")?;
                    bit_offset = (end + 1) * 8; // skip null byte
                }
            }
            Type::Optional(opt) => {
                let condition = &opt.condition;
                let inner_ty = opt.ty.first().unwrap();
                let temp_ident = Ident::new(&format!("{}_temp", ident), proc_macro2::Span::call_site());
                let inner_parse = inner_ty.generate_parse_code(&temp_ident);
                
                let peek_ident = Ident::new("peek", proc_macro2::Span::call_site());
                quote! {
                    let #ident = {
                        let condition_result = {
                            let #peek_ident = |bits: usize| -> u64 {
                                let mut temp_offset = bit_offset;
                                read_bits(data, &mut temp_offset, bits).unwrap_or(0)
                            };
                            #condition
                        };
                        if condition_result {
                            #inner_parse
                            Some(#temp_ident)
                        } else {
                            None
                        }
                    };
                }
            }
            Type::Box(inner) => {
                let temp_ident = Ident::new(&format!("{}_temp", ident), proc_macro2::Span::call_site());
                let inner_parse = inner.generate_parse_code(&temp_ident);
                quote! {
                    #inner_parse
                    let #ident = alloc::boxed::Box::new(#temp_ident);
                }
            }
            Type::Exclude(_) => {
                quote! {
                    let #ident = core::default::Default::default();
                }
            }
            Type::Custom(ty) => {
                quote! {
                    let #ident = <#ty as network_parser_rs::NetworkParse>::parse_bits(data, &mut bit_offset)?;
                }
            }
        }
    }



    fn generate_write_code(&self, accessor: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
        match self {
            Type::UInt(bits) => {
                let bits_lit = proc_macro2::Literal::u8_unsuffixed(*bits);
                quote! {
                    write_bits(&mut buffer, &mut bit_offset, #bits_lit as usize, (#accessor) as u64);
                }
            }
            Type::Int(bits) => {
                let bits_lit = proc_macro2::Literal::u8_unsuffixed(*bits);
                quote! {
                    let val = ((#accessor) as u64) & ((1 << #bits_lit) - 1);
                    write_bits(&mut buffer, &mut bit_offset, #bits_lit as usize, val);
                }
            }
            Type::Bool => {
                quote! {
                    write_bits(&mut buffer, &mut bit_offset, 1, if #accessor { 1 } else { 0 });
                }
            }
            Type::USize => {
                quote! {
                    let bits = core::mem::size_of::<usize>() * 8;
                    write_bits(&mut buffer, &mut bit_offset, bits, (#accessor) as u64);
                }
            }
            Type::Slice(len) => {
                let len_lit = proc_macro2::Literal::u8_unsuffixed(*len);
                quote! {
                    if bit_offset % 8 != 0 { panic!("Unaligned byte write for slice"); }
                    buffer.extend_from_slice(&(#accessor));
                    bit_offset += (#len_lit as usize) * 8;
                }
            }
            Type::Vec(_) => {
                quote! {
                    if bit_offset % 8 != 0 { panic!("Unaligned byte write for Vec"); }
                    buffer.extend_from_slice(&(#accessor));
                    bit_offset += (#accessor).len() * 8;
                }
            }
            Type::CStr(_) => {
                quote! {
                    if bit_offset % 8 != 0 { panic!("Unaligned byte write for CStr"); }
                    let bytes = (#accessor).as_bytes_with_nul();
                    buffer.extend_from_slice(bytes);
                    bit_offset += bytes.len() * 8;
                }
            }
            Type::Optional(opt) => {
                let inner_ty = opt.ty.first().unwrap();
                let inner_write = inner_ty.generate_write_code(&quote!(*v));
                quote! {
                    if let Some(v) = &(#accessor) {
                        #inner_write
                    }
                }
            }
            Type::Box(inner) => {
                let inner_write = inner.generate_write_code(&quote!(**v));
                quote! {
                    {
                        let v = &(#accessor);
                        #inner_write
                    }
                }
            }
            Type::Exclude(_) => {
                quote! {}
            }
            Type::Custom(ty) => {
                quote! {
                    <#ty as network_parser_rs::NetworkParse>::write_bits(&(#accessor), &mut buffer, &mut bit_offset);
                }
            }
        }
    }
}

impl Parse for Type {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(syn::token::Bracket) {
            let content;
            syn::bracketed!(content in input);
            let inner_ty: Ident = content.parse()?;
            if inner_ty.to_string() != "u8" {
                return Err(syn::Error::new(inner_ty.span(), "Only u8 is supported in slice"));
            }
            content.parse::<syn::Token![;]>()?;
            let len: LitInt = content.parse()?;
            return Ok(Type::Slice(len.base10_parse()?));
        }

        let fork = input.fork();
        if let Ok(ident) = fork.parse::<Ident>() {
            let ident_str = ident.to_string();

            if ident_str == "bool" {
                input.parse::<Ident>()?; // advance
                return Ok(Type::Bool);
            } else if ident_str == "usize" {
                input.parse::<Ident>()?; // advance
                return Ok(Type::USize);
            } else if ident_str.starts_with('u') && ident_str.len() > 1 {
                if let Ok(num) = ident_str[1..].parse::<u8>() {
                    input.parse::<Ident>()?; // advance
                    return Ok(Type::UInt(num));
                }
            } else if ident_str.starts_with('i') && ident_str.len() > 1 {
                if let Ok(num) = ident_str[1..].parse::<u8>() {
                    input.parse::<Ident>()?; // advance
                    return Ok(Type::Int(num));
                }
            }

            if ident_str == "Vec" {
                input.parse::<Ident>()?; // advance
                if input.peek(syn::token::Lt) {
                    input.parse::<syn::Token![<]>()?;
                    let inner_ty: Ident = input.parse()?;
                    if inner_ty.to_string() != "u8" {
                        return Err(syn::Error::new(inner_ty.span(), "Only u8 is supported in Vec"));
                    }
                    input.parse::<syn::Token![>]>()?;
                }
                
                let mut size_field = None;
                if input.peek(syn::Token![;]) {
                    input.parse::<syn::Token![;]>()?;
                    let expr: Expr = input.parse()?;
                    size_field = Some(expr);
                }
                return Ok(Type::Vec(size_field));
            } else if ident_str == "Option" {
                input.parse::<Ident>()?; // advance
                input.parse::<syn::Token![<]>()?;
                let inner_ty: Type = input.parse()?;
                input.parse::<syn::Token![>]>()?;
                
                let opt_type = OptionType {
                    ty: Box::new(vec![inner_ty]),
                    condition: quote!(true),
                };
                return Ok(Type::Optional(opt_type));
            } else if ident_str == "Box" {
                input.parse::<Ident>()?; // advance
                input.parse::<syn::Token![<]>()?;
                let inner_ty: Type = input.parse()?;
                input.parse::<syn::Token![>]>()?;
                return Ok(Type::Box(Box::new(inner_ty)));
            }

            if ident_str == "CStr" {
                input.parse::<Ident>()?; // advance
                if input.peek(syn::Token![;]) {
                    input.parse::<syn::Token![;]>()?;
                    let _size_field: Ident = input.parse()?;
                }
                return Ok(Type::CStr(()));
            }
        }

        if let Ok(custom_ty) = input.parse::<syn::Type>() {
            return Ok(Type::Custom(custom_ty));
        }

        Err(input.error("Unknown type: Expected a valid primitive (e.g. u8, i16, Vec<u8>, CStr) or a custom type implementing NetworkParse"))
    }
}

impl Parse for SyntaxTree {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let _vis = input.parse::<syn::Visibility>()?;
        let struct_name = Ident::parse_any(input)?;
        
        let struct_content;
        syn::braced!(struct_content in input);

        let mut inner = Vec::new();

        while !struct_content.is_empty() {
            let input = &struct_content;
            let _field_attrs = input.call(syn::Attribute::parse_outer)?;

            if input.peek(syn::Token![if]) {
                input.parse::<syn::Token![if]>()?;
                let condition = input.parse::<Expr>()?;
                
                let content;
                syn::braced!(content in input);
                
                // Fields inside the if block are wrapped in Type::Optional
                let mut if_rows = Vec::new();
                while !content.is_empty() {
                    let _inner_attrs = content.call(syn::Attribute::parse_outer)?;
                    let _vis = content.parse::<syn::Visibility>()?;

                    let fork = content.fork();
                    if let Ok(ident) = Ident::parse_any(&fork) {
                        if ident.to_string() == "consume" {
                            Ident::parse_any(&content)?; // consume
                            let inner_content;
                            syn::parenthesized!(inner_content in content);
                            let consume_type: ConsumeType = inner_content.parse()?;
                            if_rows.push(Row::Consume(consume_type));
                            
                            while content.peek(syn::Token![,]) || content.peek(syn::Token![;]) {
                                if content.peek(syn::Token![,]) {
                                    content.parse::<syn::Token![,]>()?;
                                } else {
                                    content.parse::<syn::Token![;]>()?;
                                }
                            }
                            continue;
                        }
                    }

                    let identifier = Ident::parse_any(&content)?;
                    content.parse::<syn::Token![:]>()?;
                    let ty: Type = content.parse()?;
                    
                    while content.peek(syn::Token![,]) || content.peek(syn::Token![;]) {
                        if content.peek(syn::Token![,]) {
                            content.parse::<syn::Token![,]>()?;
                        } else {
                            content.parse::<syn::Token![;]>()?;
                        }
                    }
                    
                    if_rows.push(Row::Field(Field {
                        identifier,
                        ty,
                    }));
                }
                inner.push(Row::IfBlock { condition: quote!(#condition), rows: if_rows });
            } else if input.peek(syn::Ident) && input.fork().parse::<Ident>().map(|id| id.to_string() == "exclude").unwrap_or(false) {
                input.parse::<Ident>()?; // exclude
                let content;
                syn::braced!(content in input);
                
                while !content.is_empty() {
                    let _inner_attrs = content.call(syn::Attribute::parse_outer)?;
                    let _vis = content.parse::<syn::Visibility>()?;
                    let identifier = Ident::parse_any(&content)?;
                    content.parse::<syn::Token![:]>()?;
                    let ty: Type = content.parse()?;
                    
                    while content.peek(syn::Token![,]) || content.peek(syn::Token![;]) {
                        if content.peek(syn::Token![,]) {
                            content.parse::<syn::Token![,]>()?;
                        } else {
                            content.parse::<syn::Token![;]>()?;
                        }
                    }
                    
                    inner.push(Row::Field(Field {
                        identifier,
                        ty: Type::Exclude(Box::new(ty)),
                    }));
                }
            } else {
                let _vis = input.parse::<syn::Visibility>()?;
                let fork = input.fork();
                if let Ok(ident) = Ident::parse_any(&fork) {
                    if ident.to_string() == "consume" {
                        Ident::parse_any(input)?; // consume
                        let content;
                        syn::parenthesized!(content in input);
                        let consume_type: ConsumeType = content.parse()?;
                        inner.push(Row::Consume(consume_type));
                    } else {
                        let identifier = Ident::parse_any(input)?;
                        input.parse::<syn::Token![:]>()?;
                        let ty: Type = input.parse()?;
                        inner.push(Row::Field(Field { identifier, ty }));
                    }
                } else {
                    return Err(input.error("Expected a field definition (e.g. `field: u8`), a `consume(N)` directive, or an `if` block"));
                }
            }

            while input.peek(syn::Token![,]) || input.peek(syn::Token![;]) {
                if input.peek(syn::Token![,]) {
                    input.parse::<syn::Token![,]>()?;
                } else {
                    input.parse::<syn::Token![;]>()?;
                }
            }
        }

        validate_unsized_vecs(&inner)?;

        Ok(SyntaxTree {
            attrs,
            inner,
            struct_name: struct_name.to_string(),
        })
    }
}

fn validate_unsized_vecs(rows: &[Row]) -> syn::Result<()> {
    let mut unsized_vec_ident: Option<Ident> = None;

    fn check_field(ident: &Ident, ty: &Type, unsized_vec_ident: &mut Option<Ident>) -> syn::Result<()> {
        match ty {
            Type::Exclude(_) => {
                // Excluded fields do not consume bytes from the packet, so they are allowed after an unsized Vec
            }
            Type::Vec(None) => {
                if let Some(prev) = unsized_vec_ident {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "Cannot define unsized `Vec<u8>` field `{}` because field `{}` is already an unsized `Vec<u8>`. Only one `Vec<u8>` of unspecified size is allowed per struct.",
                            ident, prev
                        ),
                    ));
                }
                *unsized_vec_ident = Some(ident.clone());
            }
            _ => {
                if let Some(prev) = unsized_vec_ident {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "Field `{}` cannot follow unsized `Vec<u8>` field `{}`. A `Vec<u8>` of unspecified size consumes all remaining data and must be the last parseable field in the struct.",
                            ident, prev
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    for row in rows {
        match row {
            Row::Field(f) => {
                check_field(&f.identifier, &f.ty, &mut unsized_vec_ident)?;
            }
            Row::Consume(c) => {
                if let Some(prev) = &unsized_vec_ident {
                    let span = match c {
                        ConsumeType::Literal(lit) => lit.span(),
                        ConsumeType::Expr(expr) => expr.span(),
                    };
                    return Err(syn::Error::new(
                        span,
                        format!(
                            "`consume` directive cannot follow unsized `Vec<u8>` field `{}`.",
                            prev
                        ),
                    ));
                }
            }
            Row::IfBlock { rows: if_rows, .. } => {
                for if_row in if_rows {
                    match if_row {
                        Row::Field(f) => {
                            check_field(&f.identifier, &f.ty, &mut unsized_vec_ident)?;
                        }
                        Row::Consume(c) => {
                            if let Some(prev) = &unsized_vec_ident {
                                let span = match c {
                                    ConsumeType::Literal(lit) => lit.span(),
                                    ConsumeType::Expr(expr) => expr.span(),
                                };
                                return Err(syn::Error::new(
                                    span,
                                    format!(
                                        "`consume` directive cannot follow unsized `Vec<u8>` field `{}`.",
                                        prev
                                    ),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}

#[proc_macro]
pub fn make_struct(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as SyntaxTree);
    let struct_name = Ident::new(&parsed.struct_name, proc_macro2::Span::call_site());
    let network_parse_impl = parsed.generate_network_parse();
    let expanded = quote! {


        #parsed
        
        #network_parse_impl
        
        impl core::convert::TryFrom<alloc::vec::Vec<u8>> for #struct_name {
            type Error = &'static str;
            fn try_from(data: alloc::vec::Vec<u8>) -> core::result::Result<Self, Self::Error> {
                let mut bit_offset = 0;
                <Self as network_parser_rs::NetworkParse>::parse_bits(&data, &mut bit_offset)
            }
        }

        impl<'a> core::convert::TryFrom<&'a [u8]> for #struct_name {
            type Error = &'static str;
            fn try_from(data: &'a [u8]) -> core::result::Result<Self, Self::Error> {
                let mut bit_offset = 0;
                <Self as network_parser_rs::NetworkParse>::parse_bits(data, &mut bit_offset)
            }
        }
        
        impl core::convert::From<#struct_name> for alloc::vec::Vec<u8> {
            fn from(val: #struct_name) -> alloc::vec::Vec<u8> {
                let mut buffer = alloc::vec::Vec::new();
                let mut bit_offset = 0;
                network_parser_rs::NetworkParse::write_bits(&val, &mut buffer, &mut bit_offset);
                buffer
            }
        }

        impl<'a> core::convert::From<&'a #struct_name> for alloc::vec::Vec<u8> {
            fn from(val: &'a #struct_name) -> alloc::vec::Vec<u8> {
                let mut buffer = alloc::vec::Vec::new();
                let mut bit_offset = 0;
                network_parser_rs::NetworkParse::write_bits(val, &mut buffer, &mut bit_offset);
                buffer
            }
        }
    };

    TokenStream::from(expanded)
}

enum VariantMatch {
    Exact(syn::Expr),
    Condition(proc_macro2::TokenStream),
    CatchAll,
    Assignment(syn::Expr, proc_macro2::TokenStream),
}

struct EnumVariant {
    ident: Ident,
    inner_type: Option<syn::Type>,
    variant_match: VariantMatch,
}

fn replace_self_with_tag_value(ts: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    ts.into_iter().map(|tt| {
        match tt {
            proc_macro2::TokenTree::Ident(ident) if ident == "self" => {
                proc_macro2::TokenTree::Ident(proc_macro2::Ident::new("tag_value", ident.span()))
            }
            proc_macro2::TokenTree::Group(group) => {
                let new_stream = replace_self_with_tag_value(group.stream());
                let mut new_group = proc_macro2::Group::new(group.delimiter(), new_stream);
                new_group.set_span(group.span());
                proc_macro2::TokenTree::Group(new_group)
            }
            _ => tt,
        }
    }).collect()
}

struct EnumDef {
    attrs: Vec<syn::Attribute>,
    enum_name: Ident,
    repr_type: Type,
    variants: Vec<EnumVariant>,
}

impl syn::parse::Parse for EnumDef {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attrs = input.call(syn::Attribute::parse_outer)?;
        let _vis = input.parse::<syn::Visibility>()?;
        let enum_name = Ident::parse_any(input)?;
        input.parse::<syn::Token![:]>()?;
        let repr_type: Type = input.parse()?;
        
        let struct_content;
        syn::braced!(struct_content in input);

        let mut variants = Vec::new();
        while !struct_content.is_empty() {
            let input = &struct_content;
            let _variant_attrs = input.call(syn::Attribute::parse_outer)?;
            let ident = Ident::parse_any(input)?;
            
            let inner_type = if input.peek(syn::token::Paren) {
                let content;
                syn::parenthesized!(content in input);
                Some(content.parse::<syn::Type>()?)
            } else {
                None
            };
            let variant_match = if input.peek(syn::Token![=]) && input.peek2(syn::Token![=]) {
                input.parse::<syn::Token![=]>()?;
                input.parse::<syn::Token![=]>()?;
                VariantMatch::Exact(input.parse::<syn::Expr>()?)
            } else if input.peek(syn::Token![=]) {
                input.parse::<syn::Token![=]>()?;
                let mut expr_tokens = proc_macro2::TokenStream::new();
                while !input.is_empty() && !input.peek(syn::Token![if]) && !input.peek(syn::Token![,]) && !input.peek(syn::Token![;]) {
                    let tt: proc_macro2::TokenTree = input.parse()?;
                    expr_tokens.extend(core::iter::once(tt));
                }
                
                let expr_tokens = replace_self_with_tag_value(expr_tokens);
                let expr = syn::parse2::<syn::Expr>(expr_tokens).expect("Invalid expression in assignment match");
                
                let mut cond_tokens = proc_macro2::TokenStream::new();
                if input.peek(syn::Token![if]) {
                    input.parse::<syn::Token![if]>()?;
                    while !input.is_empty() && !input.peek(syn::Token![,]) && !input.peek(syn::Token![;]) {
                        let tt: proc_macro2::TokenTree = input.parse()?;
                        cond_tokens.extend(core::iter::once(tt));
                    }
                } else {
                    cond_tokens.extend(quote!(true));
                }
                VariantMatch::Assignment(expr, replace_self_with_tag_value(cond_tokens))
            } else if input.peek(syn::Token![_]) {
                input.parse::<syn::Token![_]>()?;
                VariantMatch::CatchAll
            } else {
                let mut cond = proc_macro2::TokenStream::new();
                while !input.is_empty() && !input.peek(syn::Token![,]) && !input.peek(syn::Token![;]) {
                    let tt: proc_macro2::TokenTree = input.parse()?;
                    cond.extend(core::iter::once(tt));
                }
                VariantMatch::Condition(replace_self_with_tag_value(cond))
            };
            
            while input.peek(syn::Token![,]) || input.peek(syn::Token![;]) {
                if input.peek(syn::Token![,]) {
                    input.parse::<syn::Token![,]>()?;
                } else {
                    input.parse::<syn::Token![;]>()?;
                }
            }
            
            variants.push(EnumVariant { ident, inner_type, variant_match });
        }
        
        Ok(EnumDef { attrs, enum_name, repr_type, variants })
    }
}

#[proc_macro]
pub fn make_enum(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as EnumDef);
    let enum_name = &parsed.enum_name;
    let repr_type = &parsed.repr_type;
    
    let mut enum_variants = Vec::new();
    let mut match_arms = Vec::new();
    let mut write_arms = Vec::new();
    
    let write_val = match repr_type {
        Type::UInt(_) | Type::Int(_) | Type::USize => quote! { (core::clone::Clone::clone(v)) as #repr_type },
        _ => quote! { core::clone::Clone::clone(v) },
    };

    for variant in &parsed.variants {
        let ident = &variant.ident;
        
        match &variant.variant_match {
            VariantMatch::Exact(value) => {
                if let Some(inner) = &variant.inner_type {
                    enum_variants.push(quote! { #ident(#inner) });
                    match_arms.push(quote! { #value => Ok(Self::#ident(core::default::Default::default())) });
                    write_arms.push(quote! { Self::#ident(_) => #value });
                } else {
                    enum_variants.push(quote! { #ident });
                    match_arms.push(quote! { #value => Ok(Self::#ident) });
                    write_arms.push(quote! { Self::#ident => #value });
                }
            }
            VariantMatch::Assignment(expr, cond) => {
                if let Some(inner) = &variant.inner_type {
                    enum_variants.push(quote! { #ident(#inner) });
                    match_arms.push(quote! { _ if #cond => Ok(Self::#ident(#expr)) });
                    write_arms.push(quote! { Self::#ident(_) => core::default::Default::default() }); // Fallback on serialize
                } else {
                    enum_variants.push(quote! { #ident });
                    match_arms.push(quote! { _ if #cond => Ok(Self::#ident) });
                    write_arms.push(quote! { Self::#ident => core::default::Default::default() });
                }
            }
            VariantMatch::Condition(cond) => {
                if let Some(inner) = &variant.inner_type {
                    enum_variants.push(quote! { #ident(#inner) });
                    match_arms.push(quote! { v if v #cond => Ok(Self::#ident(v as #inner)) });
                    write_arms.push(quote! { Self::#ident(v) => #write_val });
                } else {
                    enum_variants.push(quote! { #ident });
                    match_arms.push(quote! { v if v #cond => Ok(Self::#ident) });
                    write_arms.push(quote! { Self::#ident => core::default::Default::default() });
                }
            }
            VariantMatch::CatchAll => {
                if let Some(inner) = &variant.inner_type {
                    enum_variants.push(quote! { #ident(#inner) });
                    match_arms.push(quote! { v => Ok(Self::#ident(v as #inner)) });
                    write_arms.push(quote! { Self::#ident(v) => #write_val });
                } else {
                    enum_variants.push(quote! { #ident });
                    match_arms.push(quote! { _ => Ok(Self::#ident) });
                    write_arms.push(quote! { Self::#ident => core::default::Default::default() });
                }
            }
        }
    }
    
    let repr_type = &parsed.repr_type;
    
    let tag_value_ident = Ident::new("tag_value", proc_macro2::Span::call_site());
    let parse_tag_value = repr_type.generate_parse_code(&tag_value_ident);
    let write_tag_value = repr_type.generate_write_code(&quote!(tag_value));

    let attrs = &parsed.attrs;

    let expanded = quote! {
        #(#attrs)*
        pub enum #enum_name {
            #(#enum_variants),*
        }
        
        impl<'a> network_parser_rs::NetworkParse<'a> for #enum_name {
            fn parse_bits(data: &[u8], bit_offset_ref: &mut usize) -> core::result::Result<Self, &'static str> {
                let mut bit_offset = *bit_offset_ref;
                let read_bits = |data: &[u8], bit_offset: &mut usize, bits: usize| -> core::result::Result<u64, &'static str> {
                    if *bit_offset + bits > data.len() * 8 {
                        return Err("EOF");
                    }
                    if *bit_offset % 8 == 0 {
                        let byte_idx = *bit_offset / 8;
                        if bits == 8 {
                            *bit_offset += 8;
                            return Ok(data[byte_idx] as u64);
                        } else if bits == 16 {
                            *bit_offset += 16;
                            let mut buf = [0u8; 2];
                            buf.copy_from_slice(&data[byte_idx..byte_idx+2]);
                            return Ok(u16::from_be_bytes(buf) as u64);
                        } else if bits == 32 {
                            *bit_offset += 32;
                            let mut buf = [0u8; 4];
                            buf.copy_from_slice(&data[byte_idx..byte_idx+4]);
                            return Ok(u32::from_be_bytes(buf) as u64);
                        } else if bits == 64 {
                            *bit_offset += 64;
                            let mut buf = [0u8; 8];
                            buf.copy_from_slice(&data[byte_idx..byte_idx+8]);
                            return Ok(u64::from_be_bytes(buf) as u64);
                        }
                    }
                    
                    let mut val: u64 = 0;
                    for i in 0..bits {
                        let current_bit = *bit_offset + i;
                        let byte_idx = current_bit / 8;
                        let bit_idx = 7 - (current_bit % 8);
                        let bit = (data[byte_idx] >> bit_idx) & 1;
                        val = (val << 1) | (bit as u64);
                    }
                    *bit_offset += bits;
                    Ok(val)
                };

                #parse_tag_value
                
                *bit_offset_ref = bit_offset;
                
                match tag_value {
                    #(#match_arms,)*
                    _ => Err("Invalid enum variant tag")
                }
            }

            fn write_bits(&self, mut buffer: &mut alloc::vec::Vec<u8>, mut bit_offset_ref: &mut usize) {
                let tag_value = match self {
                    #(#write_arms),*
                };

                let mut bit_offset = *bit_offset_ref;
                let mut write_bits = |buffer: &mut alloc::vec::Vec<u8>, bit_offset: &mut usize, bits: usize, val: u64| {
                    if *bit_offset % 8 == 0 {
                        let byte_idx = *bit_offset / 8;
                        if bits == 8 {
                            while buffer.len() <= byte_idx { buffer.push(0); }
                            buffer[byte_idx] = val as u8;
                            *bit_offset += 8;
                            return;
                        } else if bits == 16 {
                            while buffer.len() <= byte_idx + 1 { buffer.push(0); }
                            buffer[byte_idx..byte_idx+2].copy_from_slice(&(val as u16).to_be_bytes());
                            *bit_offset += 16;
                            return;
                        } else if bits == 32 {
                            while buffer.len() <= byte_idx + 3 { buffer.push(0); }
                            buffer[byte_idx..byte_idx+4].copy_from_slice(&(val as u32).to_be_bytes());
                            *bit_offset += 32;
                            return;
                        } else if bits == 64 {
                            while buffer.len() <= byte_idx + 7 { buffer.push(0); }
                            buffer[byte_idx..byte_idx+8].copy_from_slice(&(val as u64).to_be_bytes());
                            *bit_offset += 64;
                            return;
                        }
                    }
                    
                    for i in 0..bits {
                        let current_bit = *bit_offset + i;
                        let byte_idx = current_bit / 8;
                        let bit_idx = 7 - (current_bit % 8);
                        
                        while buffer.len() <= byte_idx { buffer.push(0); }
                        
                        let bit = (val >> (bits - 1 - i)) & 1;
                        if bit == 1 {
                            buffer[byte_idx] |= 1 << bit_idx;
                        } else {
                            buffer[byte_idx] &= !(1 << bit_idx);
                        }
                    }
                    *bit_offset += bits;
                };

                #write_tag_value
                *bit_offset_ref = bit_offset;
            }
        }
        
        impl core::convert::TryFrom<alloc::vec::Vec<u8>> for #enum_name {
            type Error = &'static str;
            fn try_from(data: alloc::vec::Vec<u8>) -> core::result::Result<Self, Self::Error> {
                let mut bit_offset = 0;
                <Self as network_parser_rs::NetworkParse>::parse_bits(&data, &mut bit_offset)
            }
        }

        impl<'a> core::convert::TryFrom<&'a [u8]> for #enum_name {
            type Error = &'static str;
            fn try_from(data: &'a [u8]) -> core::result::Result<Self, Self::Error> {
                let mut bit_offset = 0;
                <Self as network_parser_rs::NetworkParse>::parse_bits(data, &mut bit_offset)
            }
        }
        
        impl core::convert::From<#enum_name> for alloc::vec::Vec<u8> {
            fn from(val: #enum_name) -> alloc::vec::Vec<u8> {
                let mut buffer = alloc::vec::Vec::new();
                let mut bit_offset = 0;
                network_parser_rs::NetworkParse::write_bits(&val, &mut buffer, &mut bit_offset);
                buffer
            }
        }

        impl<'a> core::convert::From<&'a #enum_name> for alloc::vec::Vec<u8> {
            fn from(val: &'a #enum_name) -> alloc::vec::Vec<u8> {
                let mut buffer = alloc::vec::Vec::new();
                let mut bit_offset = 0;
                network_parser_rs::NetworkParse::write_bits(val, &mut buffer, &mut bit_offset);
                buffer
            }
        }
    };
    
    TokenStream::from(expanded)
}
