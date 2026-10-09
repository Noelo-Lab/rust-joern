int sink(int a, int b);
int known(int x);
int ordinary_blocks(int x) { { x++; } return x; }
int ordinary_call_block(int x) { known(x); { x++; } return x; }
int unknown_call_brace(int x) { mystery(x) { x++; } return x; }
int known_call_brace(int x) { known(x) { x++; } return x; }
int recursive_call_brace(int x) { recursive_call_brace(x) { x++; } return x; }
int condition_call_brace(int x) { mystery(x && known(x)) { x++; } return x; }
int nested_do_call_brace(int x) { do { mystery(x != 0) { known(x); x++; } } while (0); return x; }
int nested_if_call_brace(int x) { if (x) { mystery(x) { x++; } x += 2; } return x; }
int scalar_if_call_brace(int x) { if (x) mystery(x) { x++; } return x; }
int scalar_while_call_brace(int x) { while (x) mystery(x) { x--; } return x; }
int return_call_brace(int x) { return mystery(x) { x+1 }; return x; }
int init_call_brace(int x) { int y=mystery(x) { x+1 }; return y; }
int arg_call_brace(int x) { sink(mystery(x) { x+1 },x); return x; }
int if_condition_call_brace(int x) { if (mystery(x) { x+1 }) x++; return x; }
int tag_assign(int x) { x=0x00000002<p32>; x++; return x; }
int tag_cast_assign(int x) { x=(int)(int*)0x00000002<p32>; x++; return x; }
int tag_init(int x) { int y=0x00000002<p32>; x++; return y; }
int tag_arg(int x) { sink(0x00000002<p32>,x); x++; return x; }
int tag_arg_two(int x) { sink(x,0x000000000000AADC<p64>); return x; }
int tag_return(int x) { return 0x00000002<p32>; return x; }
int tag_if(int x) { if (0x00000002<p32>) x++; return x; }
int tag_and_if(int x) { if (x && 0x00000002<p32>) x++; return x; }
int tag_while(int x) { while (0x00000002<p32>) x--; return x; }
int tag_scalar_body(int x) { if (x) x=0x00000002<p32>; return x; }
int tag_braced_body(int x) { if (x) { x=0x00000002<p32>; x++; } return x; }
int valid_comparison(int p32,int x) { x=1<p32>0; return x; }
int valid_comparison_rhs(int p32,int x) { x=0x2<p32>+x; return x; }
int valid_cast_address(int x) { x=(int)(int*)0x00000002; return x; }
int valid_compound_literal(int x) { int y=(int){x}; return y; }
struct Pair { int x; int y; };
int valid_aggregate(int x) { struct Pair p={x,2}; return p.x; }
int valid_function_body(int x) { if(x && known(x)) {x++;} return x; }
