

pub fn find_closing_parrent(parrent:char,str:String)->usize{
    let opposite_parrent=match parrent{
        '('=>')',
        '['=>']',
        '{'=>'}',
        '<'=>'>',
        _=>panic!("invalid parrent")
    };
    let mut i=0;
    for (ind,c) in str.chars().enumerate(){
        if c==parrent{
            i+=1;
        }
        else if c==opposite_parrent{
            i-=1;
            if i==0{
                return ind;
            }
        }

    }
    return 0;

}

#[test]
pub fn test_find_closing_parrent(){
    assert_eq!(find_closing_parrent('(',String::from("fresfwadaw(freshgsrkjfe(fesagsruih)({[}dwa}daw>dvgtg<>])efsgtdb)hdf")),63);
}
