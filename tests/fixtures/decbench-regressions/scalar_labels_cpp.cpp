int scalar_while(int x) { while(x) again:x--; return x; }
int scalar_do(int x) { do again:x--; while(x); return x; }
int nested_while(int x) { while(x) a:b:x--; return x; }
int while_goto(int x) { while(x) again:if(--x) goto again; return x; }
int scalar_switch(int x) { switch(x) again:x--; return x; }
int nested_switch(int x) { switch(x) a:b:x--; return x; }
int switch_case_label(int x) { switch(x) case 1:again:x--; return x; }
