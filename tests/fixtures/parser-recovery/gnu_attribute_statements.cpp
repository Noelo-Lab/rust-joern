int call(int x);
int gnu_standalone(int x) { x++; __attribute__((fallthrough)); return x; }
int gnu_scalar_if(int x) { if (x) __attribute__((fallthrough)); return x; }
int gnu_scalar_else(int x) { if (x) x++; else __attribute__((fallthrough)); return x; }
int gnu_scalar_while(int x) { while (x) __attribute__((fallthrough)); return x; }
int gnu_label(int x) { label: __attribute__((fallthrough)); return x; }
int gnu_switch(int x) { switch (x) { case 1: if (x>1) { call(1); break; } ;__attribute__ ((fallthrough)); default: call(2); } return x; }
int gnu_switch_plain(int x) { switch (x) { case 1: if (x>1) { call(1); break; } default: call(2); } return x; }
int gnu_attribute_prefix(int x) { __attribute__((unused)) call(x); return x; }
int gnu_unknown_attribute(int x) { x++; __attribute__((made_up)); return x; }
