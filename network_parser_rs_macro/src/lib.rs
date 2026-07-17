mod utils;

#[allow(dead_code)] //todo
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{Expr, LitInt, Ident};
use syn::parse::{Parse, ParseStream};
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
    CStr(()),
    Slice(u8),
    Optional(OptionType), // field defined behind if condition
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
    Field(Field),
}

struct Field{
    identifier: Ident,
    ty: Type,
}

struct SyntaxTree{
    pub inner: Vec<Row>,
    pub struct_name: String
}
impl ToTokens for SyntaxTree {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let struct_name = Ident::new(&self.struct_name, proc_macro2::Span::call_site());
        let fields = self.inner.iter().filter_map(|row| match row {
            Row::Field(f) => Some(quote!(#f)),
            Row::Consume(_) => None,
        });

        tokens.extend(quote! {
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
            Type::Vec(_) => { tokens.extend(quote!(Vec<u8>)); }
            Type::CStr(_) => { tokens.extend(quote!(std::ffi::CString)); }
            Type::Slice(len) => {
                let len_lit = proc_macro2::Literal::u8_unsuffixed(*len);
                tokens.extend(quote!([u8; #len_lit]));
            }
            Type::Optional(opt) => {
                let inner_ty = opt.ty.first().unwrap();
                tokens.extend(quote!(Option<#inner_ty>));
            }
        }
    }
}

impl SyntaxTree {
    fn generate_try_from(&self) -> proc_macro2::TokenStream {
        let struct_name = Ident::new(&self.struct_name, proc_macro2::Span::call_site());
        
        let mut field_reads = Vec::new();
        let mut struct_fields = Vec::new();

        for row in &self.inner {
            match row {
                Row::Consume(c) => {
                    let amount = match c {
                        ConsumeType::Literal(lit) => quote!(#lit as usize),
                        ConsumeType::Expr(expr) => quote!((#expr) as usize),
                    };
                    field_reads.push(quote! {
                        bit_offset += #amount;
                    });
                }
                Row::Field(f) => {
                    let ident = &f.identifier;
                    let ty = &f.ty;
                    let parse_code = ty.generate_parse_code(ident);
                    field_reads.push(parse_code);
                    struct_fields.push(ident);
                }
            }
        }

        quote! {
            impl TryFrom<Vec<u8>> for #struct_name {
                type Error = &'static str;

                fn try_from(data: Vec<u8>) -> Result<Self, Self::Error> {
                    let mut bit_offset: usize = 0;
                    
                    let read_bits = |data: &[u8], bit_offset: &mut usize, bits: usize| -> Result<u64, &'static str> {
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

                    #(#field_reads)*

                    Ok(Self {
                        #(#struct_fields,)*
                    })
                }
            }
        }
    }

    fn generate_from_ptr(&self) -> proc_macro2::TokenStream {
        let struct_name = Ident::new(&self.struct_name, proc_macro2::Span::call_site());
        
        let mut field_reads = Vec::new();
        let mut struct_fields = Vec::new();

        for row in &self.inner {
            match row {
                Row::Consume(c) => {
                    let amount = match c {
                        ConsumeType::Literal(lit) => quote!(#lit as usize),
                        ConsumeType::Expr(expr) => quote!((#expr) as usize),
                    };
                    field_reads.push(quote! {
                        bit_offset += #amount;
                    });
                }
                Row::Field(f) => {
                    let ident = &f.identifier;
                    let ty = &f.ty;
                    let parse_code = ty.generate_parse_code_ptr(ident);
                    field_reads.push(parse_code);
                    struct_fields.push(ident);
                }
            }
        }

        quote! {
            impl From<*mut u8> for #struct_name {
                fn from(ptr: *mut u8) -> Self {
                    let mut bit_offset: usize = 0;
                    
                    let read_bits_ptr = |ptr: *const u8, bit_offset: &mut usize, bits: usize| -> u64 {
                        if *bit_offset % 8 == 0 {
                            let byte_idx = *bit_offset / 8;
                            if bits == 8 {
                                *bit_offset += 8;
                                return unsafe { *ptr.add(byte_idx) as u64 };
                            } else if bits == 16 {
                                *bit_offset += 16;
                                let mut buf = [0u8; 2];
                                unsafe { std::ptr::copy_nonoverlapping(ptr.add(byte_idx), buf.as_mut_ptr(), 2); }
                                return u16::from_be_bytes(buf) as u64;
                            } else if bits == 32 {
                                *bit_offset += 32;
                                let mut buf = [0u8; 4];
                                unsafe { std::ptr::copy_nonoverlapping(ptr.add(byte_idx), buf.as_mut_ptr(), 4); }
                                return u32::from_be_bytes(buf) as u64;
                            } else if bits == 64 {
                                *bit_offset += 64;
                                let mut buf = [0u8; 8];
                                unsafe { std::ptr::copy_nonoverlapping(ptr.add(byte_idx), buf.as_mut_ptr(), 8); }
                                return u64::from_be_bytes(buf) as u64;
                            }
                        }
                        
                        let mut val: u64 = 0;
                        for i in 0..bits {
                            let current_bit = *bit_offset + i;
                            let byte_idx = current_bit / 8;
                            let bit_idx = 7 - (current_bit % 8);
                            let bit = unsafe { (*ptr.add(byte_idx) >> bit_idx) & 1 };
                            val = (val << 1) | (bit as u64);
                        }
                        *bit_offset += bits;
                        val
                    };

                    #(#field_reads)*

                    Self {
                        #(#struct_fields,)*
                    }
                }
            }
        }
    }

    fn generate_into(&self) -> proc_macro2::TokenStream {
        let struct_name = Ident::new(&self.struct_name, proc_macro2::Span::call_site());
        
        let mut field_writes = Vec::new();

        for row in &self.inner {
            match row {
                Row::Consume(c) => {
                    let amount = match c {
                        ConsumeType::Literal(lit) => quote!(#lit as usize),
                        ConsumeType::Expr(expr) => quote!((#expr) as usize),
                    };
                    field_writes.push(quote! {
                        write_bits(&mut buffer, &mut bit_offset, #amount, 0);
                    });
                }
                Row::Field(f) => {
                    let ident = &f.identifier;
                    let ty = &f.ty;
                    let write_code = ty.generate_write_code(&quote!(self.#ident));
                    field_writes.push(write_code);
                }
            }
        }

        quote! {
            impl Into<Vec<u8>> for #struct_name {
                fn into(self) -> Vec<u8> {
                    let mut buffer: Vec<u8> = Vec::new();
                    let mut bit_offset: usize = 0;
                    
                    let mut write_bits = |buffer: &mut Vec<u8>, bit_offset: &mut usize, bits: usize, val: u64| {
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

                    #(#field_writes)*
                    
                    buffer
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
                if let Some(size_field) = size_opt {
                    quote! {
                        if bit_offset % 8 != 0 { return Err("Unaligned byte read for Vec"); }
                        let byte_offset = bit_offset / 8;
                        let len = #size_field as usize;
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
                    let #ident = std::ffi::CString::new(data[byte_offset..end].to_vec())
                        .map_err(|_| "Invalid CStr")?;
                    bit_offset = (end + 1) * 8; // skip null byte
                }
            }
            Type::Optional(opt) => {
                let condition = &opt.condition;
                let inner_ty = opt.ty.first().unwrap();
                let temp_ident = Ident::new(&format!("{}_temp", ident), proc_macro2::Span::call_site());
                let inner_parse = inner_ty.generate_parse_code(&temp_ident);
                
                quote! {
                    let #ident = if #condition {
                        #inner_parse
                        Some(#temp_ident)
                    } else {
                        None
                    };
                }
            }
        }
    }

    fn generate_parse_code_ptr(&self, ident: &Ident) -> proc_macro2::TokenStream {
        match self {
            Type::UInt(bits) => {
                let bits_lit = proc_macro2::Literal::u8_unsuffixed(*bits);
                let rust_ty = if *bits <= 8 { quote!(u8) } 
                    else if *bits <= 16 { quote!(u16) } 
                    else if *bits <= 32 { quote!(u32) } 
                    else { quote!(u64) };
                
                quote! {
                    let #ident = read_bits_ptr(ptr, &mut bit_offset, #bits_lit as usize) as #rust_ty;
                }
            }
            Type::Int(bits) => {
                let bits_lit = proc_macro2::Literal::u8_unsuffixed(*bits);
                let rust_ty = if *bits <= 8 { quote!(i8) } 
                    else if *bits <= 16 { quote!(i16) } 
                    else if *bits <= 32 { quote!(i32) } 
                    else { quote!(i64) };

                quote! {
                    let raw = read_bits_ptr(ptr, &mut bit_offset, #bits_lit as usize);
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
                    if bit_offset % 8 != 0 { panic!("Unaligned byte read for slice"); }
                    let byte_offset = bit_offset / 8;
                    let mut #ident = [0u8; #len_lit as usize];
                    unsafe {
                        std::ptr::copy_nonoverlapping(ptr.add(byte_offset), #ident.as_mut_ptr(), #len_lit as usize);
                    }
                    bit_offset += (#len_lit as usize) * 8;
                }
            }
            Type::Vec(size_opt) => {
                if let Some(size_field) = size_opt {
                    quote! {
                        if bit_offset % 8 != 0 { panic!("Unaligned byte read for Vec"); }
                        let byte_offset = bit_offset / 8;
                        let len = #size_field as usize;
                        let mut #ident = Vec::with_capacity(len);
                        unsafe {
                            #ident.set_len(len);
                            std::ptr::copy_nonoverlapping(ptr.add(byte_offset), #ident.as_mut_ptr(), len);
                        }
                        bit_offset += len * 8;
                    }
                } else {
                    quote! {
                        compile_error!("Cannot read an unbounded Vec<u8> from a raw pointer");
                    }
                }
            }
            Type::CStr(_) => {
                quote! {
                    if bit_offset % 8 != 0 { panic!("Unaligned byte read for CStr"); }
                    let byte_offset = bit_offset / 8;
                    let mut end = byte_offset;
                    let #ident = unsafe {
                        while *ptr.add(end) != 0 {
                            end += 1;
                        }
                        let slice = std::slice::from_raw_parts(ptr.add(byte_offset), end - byte_offset);
                        std::ffi::CString::new(slice).expect("Invalid CStr")
                    };
                    bit_offset = (end + 1) * 8; // skip null byte
                }
            }
            Type::Optional(opt) => {
                let condition = &opt.condition;
                let inner_ty = opt.ty.first().unwrap();
                let temp_ident = Ident::new(&format!("{}_temp", ident), proc_macro2::Span::call_site());
                let inner_parse = inner_ty.generate_parse_code_ptr(&temp_ident);
                
                quote! {
                    let #ident = if #condition {
                        #inner_parse
                        Some(#temp_ident)
                    } else {
                        None
                    };
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

        let ident: Ident = input.parse()?;
        let ident_str = ident.to_string();

        if ident_str.starts_with('u') && ident_str.len() > 1 {
            if let Ok(num) = ident_str[1..].parse::<u8>() {
                return Ok(Type::UInt(num));
            }
        } else if ident_str.starts_with('i') && ident_str.len() > 1 {
            if let Ok(num) = ident_str[1..].parse::<u8>() {
                return Ok(Type::Int(num));
            }
        }

        if ident_str == "Vec" {
            input.parse::<syn::Token![<]>()?;
            let inner_ty: Ident = input.parse()?;
            if inner_ty.to_string() != "u8" {
                return Err(syn::Error::new(inner_ty.span(), "Only u8 is supported in Vec"));
            }
            input.parse::<syn::Token![>]>()?;
            let mut size_field = None;
            if input.peek(syn::Token![;]) {
                input.parse::<syn::Token![;]>()?;
                size_field = Some(input.parse()?);
            }
            return Ok(Type::Vec(size_field));
        }

        if ident_str == "CStr" {
            if input.peek(syn::Token![;]) {
                input.parse::<syn::Token![;]>()?;
                let _size_field: Ident = input.parse()?;
            }
            return Ok(Type::CStr(()));
        }

        Err(syn::Error::new(ident.span(), "Unknown type"))
    }
}

impl Parse for SyntaxTree {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;
        input.parse::<syn::Token![,]>()?;

        let mut inner = Vec::new();

        while !input.is_empty() {
            if input.peek(syn::Token![if]) {
                input.parse::<syn::Token![if]>()?;
                let condition = input.parse::<Expr>()?;
                
                let content;
                syn::braced!(content in input);
                
                // Fields inside the if block are wrapped in Type::Optional
                while !content.is_empty() {
                    let identifier: Ident = content.parse()?;
                    content.parse::<syn::Token![:]>()?;
                    let ty: Type = content.parse()?;
                    
                    if content.peek(syn::Token![,]) {
                        content.parse::<syn::Token![,]>()?;
                    }
                    
                    let opt_type = OptionType {
                        ty: Box::new(vec![ty]),
                        condition: quote!(#condition),
                    };
                    
                    inner.push(Row::Field(Field {
                        identifier,
                        ty: Type::Optional(opt_type),
                    }));
                }
            } else if input.fork().parse::<Ident>().is_ok() {
                let fork = input.fork();
                let ident: Ident = fork.parse()?;
                
                if ident.to_string() == "consume" {
                    input.parse::<Ident>()?; // consume
                    let content;
                    syn::parenthesized!(content in input);
                    let consume_type: ConsumeType = content.parse()?;
                    inner.push(Row::Consume(consume_type));
                } else {
                    let identifier: Ident = input.parse()?;
                    input.parse::<syn::Token![:]>()?;
                    let ty: Type = input.parse()?;
                    inner.push(Row::Field(Field { identifier, ty }));
                }
            } else {
                return Err(input.error("Expected field, consume, or if block"));
            }

            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
        }

        Ok(SyntaxTree {
            inner,
            struct_name: struct_name.to_string(),
        })
    }
}

impl From<TokenStream> for SyntaxTree {
    fn from(value: TokenStream) -> Self {
        syn::parse(value).expect("Failed to parse SyntaxTree")
    }
}
#[proc_macro]
pub fn make_struct(input: TokenStream) -> TokenStream {
    let parsed: SyntaxTree = input.into();
    let struct_name = Ident::new(&parsed.struct_name, proc_macro2::Span::call_site());

    let try_from_impl = parsed.generate_try_from();
    let from_ptr_impl = parsed.generate_from_ptr();
    let into_impl = parsed.generate_into();

    let expanded = quote! {
        #parsed
        
        #try_from_impl
        #from_ptr_impl
        #into_impl
        
        impl NetworkParse for #struct_name {}
    };

    TokenStream::from(expanded)
}