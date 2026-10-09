int work(int);
int broken_nested_body(int x) { if (work("bad
 { x++; } return x; }
int after_nested_body(int x) { return work(x); }
int broken_in_block(int x) { if(x) { if (work("bad
 x++; } work(x); return x; }
int after_nested_block(int x) { return work(x); }
int broken_while(int x) { while (work("bad
 x++; return x; }
int broken_switch(int x) { switch (work("bad
 { case 1: x++; } return x; }
int valid_escaped_literal(int x) { return work("line1\
line2"); }
int after_all_literal_conditions(int x) { if(x) work(x); return x; }
