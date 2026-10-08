int for_all(int x) { for(x=0;x<3;x++) again:x--; return x; }
int for_no_condition(int x) { for(x=0;;x++) again:x--; return x; }
int for_no_update(int x) { for(x=0;x<3;) again:x--; return x; }
int for_empty(int x) { for(;;) again:x--; return x; }
int switch_cases(int x) { switch(x) case 1:case 2:again:x--; return x; }
int switch_default(int x) { switch(x) default:again:x--; return x; }
int while_declaration(int x) { while(x) int y=x--; return x; }
