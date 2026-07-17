use network_parser_rs::make_struct;
#[test]
#[allow(unused)]
pub fn exec(){
    make_struct! { MyStruct   ,
        field1: u3,
        consume(5) //skips 5 bytes
        /*field2: u5,
        field3: Vec<u8>;field1, // vec of size defined in field1
        field4: CStr,*/
        if peek(u8)==15{
            field5: u16,
            //field 5 will be defined as Option<u16> filled only when the next two bytes are 15
        }
        field6: [u8;4],
    }

}
