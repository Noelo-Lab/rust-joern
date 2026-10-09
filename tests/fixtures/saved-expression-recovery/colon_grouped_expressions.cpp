typedef unsigned int uint32_t;
int call(int);
int valid_deref(int x){x=*(int *)0x28;return x;}
int colon_assignment(int x){x=*(fs:0x28);return x;}
int colon_type(int x){x=(uint32_t:0x28);return x;}
int colon_return(int x){return *(fs:0x28);}
int colon_initializer(int x){int y=*(fs:0x28);return x;}
int colon_condition(int x){if(*(fs:0x28))x++;return x;}
int colon_mixed_condition(int x){if(x&&*(fs:0x28))x++;return x;}
int colon_scalar_if(int x){if(x)x=*(fs:0x28);return x;}
int colon_braced_if(int x){if(x){x=*(fs:0x28);}return x;}
int colon_while(int x){while(x)x=*(fs:0x28);return x;}
int colon_combined(int x){x++,x=*(fs:0x28);return x;}
int colon_argument(int x){call(*(fs:0x28));return x;}
int conditional(int x){x=x?1:2;return x;}
