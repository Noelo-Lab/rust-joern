void target(void);
void cb(void (*f)(void));
int cycle(int x) {
again:
 if(x && x--) goto again;
 return x;
}
int hidden_cycle(void) { again: goto again; }
int for_slots(int x) { for(x=0;;++x) { if(x>3) break; x++; } return x; }
int short_circuit(int x) { return (x && cycle(x)) || cycle(x+1); }
int multiline_ref(int x) {
 if(x) cb(
  target);
 return x;
}
struct Foo { int x; };
int Foo(int x) { return x; }
int type_tag(int x) { if(sizeof(struct Foo) && Foo(x)) return x; return 0; }
int direct_comma(int x) { if(x++,x) x++; for(;x++,x;) x--; switch(x++,x) {case 1: x++;} return x; }
int paren_comma(int x) { if((x++,x)) x++; for(;(x++,x);) x--; switch((x++,x)) {case 1: x++;} return x; }
