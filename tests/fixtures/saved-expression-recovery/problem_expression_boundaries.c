int get(void);
int numeric_adjacent(int x){return 1 "literal";}
int sizeof_string(void){return sizeof "literal";}
int known_cast(void){return (int)"literal";}
int unknown_cast(void){return (X)"literal";}
int call_adjacent(void){return get()"literal";}
int suffix_plain(void){return "literal"s;}
int suffix_underscore(void){return "literal"_tag;}
int valid_colon_nested(int x){return (x?1:2);}
int invalid_colon_nested(int x){return x?(fs:0x28):1;}
int omitted_middle(int x){return x?:1;}
int type_bitfield(int x){return sizeof(struct {int bits:3;});}
int asm_colons(int x){asm("nop" : : "r"(x) : "memory");return x;}
