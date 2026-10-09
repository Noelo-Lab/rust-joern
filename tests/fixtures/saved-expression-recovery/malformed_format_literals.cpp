int printf(const char *, ...);
int adjacent(int x){printf("first" "second",x);return x;}
int plus(int x){printf("first"+"second",x);return x;}
int bad_percent(int x){printf("Moving %s "%s" (%u) to before "+" entry\n",x,x,x);return x;}
int bad_identifier(int x){printf("first"name"second",x);return x;}
int bad_return(int x){return printf("Adding %s "%s" (%u)\n",x,x,x);}
int bad_initializer(int x){int y=printf("Adding %s "%s" (%u)\n",x,x,x);return x;}
int bad_condition(int x){if(printf("Adding %s "%s" (%u)\n",x,x,x))x++;return x;}
int bad_scalar_if(int x){if(x)printf("Adding %s "%s" (%u)\n",x,x,x);return x;}
int bad_braced_if(int x){if(x){printf("Adding %s "%s" (%u)\n",x,x,x);}return x;}
int bad_combined(int x){x++,printf("Adding %s "%s" (%u)\n",x,x,x);return x;}
int valid_format(int x){printf("Adding %s \"%s\" (%u)\n",x,x,x);return x;}
int string_modulo(int x){printf("first"%x,x);return x;}
