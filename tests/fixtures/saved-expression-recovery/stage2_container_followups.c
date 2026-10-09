int known(int x);
int else_bad_consequence(int x) { if(x) mystery(x){x++;} else {x-=2;} return x; }
int else_bad_alternative(int x) { if(x) known(x); else mystery(x){x--;} return x; }
int else_both_bad(int x) { if(x) mystery(x){x++;} else mystery(x){x--;} return x; }
int for_scalar_head(int x) { for(x=0;x<3;x++) mystery(x){x--;} return x; }
int do_scalar_head(int x) { do mystery(x){x--;} while(x); return x; }
int case_scalar_head(int x) { switch(x) case 1: mystery(x){x++;} return x; }
int label_scalar_head(int x) { goto label; label: mystery(x){x++;} return x; }
int while_label_head(int x) { while(x) label: mystery(x){x--;} return x; }
int missing_assign_separator(int x) { x=known(x) known(x); x++; return x; }
int missing_call_separator(int x) { known(x) known(x); x++; return x; }
int malformed_prefix_separator(int x) { unknown(<bad>) known(x); x++; return x; }
int valid_semicolon_separators(int x) { x=known(x); known(x); x++; return x; }
int valid_scalar_controls(int x) { if(x)known(x);else known(x); while(x)known(x); for(x=0;x<3;x++)known(x); do known(x);while(x); return x; }
int newline_call_separator(int x) {
 known(x)
 known(x);
 return x;
}
