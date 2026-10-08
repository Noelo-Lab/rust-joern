void sink(int);
int unreachable(int x) { return x; sink(x); }
int second_return(int x) { return x; return x+1; }
int never(int x) { while(1) { x++; } return x; }
int multi_exit(int x, int y) { if (x) return y; if (y) return x; sink(x); }
int goto_unreachable(int x) { goto L; sink(x); L: return x; }
int dead_call(int x) { if (0) sink(x); return x; }
int switch_out(int x) { switch(x) { case 0: return x; case 1: x++; break; default: x--; } return x; }
int empty_body(int x) {}
int bare_return(int x) { return; }
int only_goto(int x) { L: goto L; }
int break_end(int x) { while(x) { sink(x); break; } }
int unset(int x) { int y; return y; }
int unknown_input(void) { sink(G); return 0; }
int global_unreachable(void) { return 1; sink(G); }
