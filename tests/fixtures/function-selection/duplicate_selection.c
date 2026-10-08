int shared(int x) { if(x>0) { while(x>1) x--; } return x; }
int shared(int x) { return x; }
int reverse(int x) { return x; }
int reverse(int x) { if(x>0) { while(x>1) x--; } return x; }
int body_then_proto(int x) { if(x) return x+1; return x; }
int body_then_proto(int x);
int proto_then_body(int x);
int proto_then_body(int x) { if(x) return x+1; return x; }
int overload(int x) { if(x>0) { while(x>1) x--; } return x; }
int overload(int x, int y) { return x+y; }
long different_return(int x) { if(x) return x+1; return x; }
int different_return(int x) { return x; }
