int known();
struct Pair{int x;};
int ordinary_assignment(int x,int y){
x={0};
return x;
}
int after_ordinary_assignment(int x,int y){
return x;
}
int label_assignment(int x,int y){
label:x={0};
return x;
}
int ordinary_return(int x,int y){
return{0};
}
int scalar_if_assignment(int x,int y){
if(x)x={0};
return x;
}
int scalar_if_return(int x,int y){
if(x)return{0};
return x;
}
int braced_if_assignment(int x,int y){
if(x){
x={0};
}
return x;
}
int braced_if_return(int x,int y){
if(x){
return{0};
}
return x;
}
int scalar_while_assignment(int x,int y){
while(x)x={0};
return x;
}
int scalar_while_return(int x,int y){
while(x)return{0};
return x;
}
int braced_while_assignment(int x,int y){
while(x){
x={0};
}
return x;
}
int braced_while_return(int x,int y){
while(x){
return{0};
}
return x;
}
int after_body_controls(int x,int y){
return x;
}
int positive_array_declaration(int x,int y){
int a[]={0};
return a[0]+x;
}
int positive_compound_literal(int x,int y){
return (struct Pair){0}.x;
}
int positive_gnu_statement_expression(int x,int y){
return ({0;});
}
int positive_scalar_assignment(int x,int y){
if(x)x=0;
return x;
}
int positive_return_zero(int x,int y){
return 0;
}
int simple_if_condition(int x,int y){
x++;
if(x!={0})x++;
return x;
}
int after_simple_if_condition(int x,int y){
return x;
}
int logical_if_condition(int x,int y){
x++;
if(y && x!={0})x++;
return x;
}
int parenthesized_if_condition(int x,int y){
x++;
if(((x*y)>>4)!={0})x++;
return x;
}
int after_parenthesized_if_condition(int x,int y){
return x;
}
int while_condition(int x,int y){
x++;
while(x!={0})x--;
return x;
}
int nested_if_condition(int x,int y){
x++;
if(x){
y++;
if(((x*y)>>4)!={0})x++;
y--;
}
return x;
}
int after_nested_if_condition(int x,int y){
return x;
}
int positive_json_quote_condition(int x,int y){
x++;
if(!known("{\"type\":\"value\",\"challenge\":\"",1)
 && !known("\",\"origin\":\"",2)){
int z=known(x);
if(known(x)&&known(x+1))x=0;
}
return x;
}
int malformed_json_quote_condition(int x,int y){
x++;
if(!known("{"type":"value","challenge":"",1)
 && !known("","origin":"",2)){
int z=known(x);
if(known(x)&&known(x+1))x=0;
}
return x;
}
int after_json_quote_condition(int x,int y){
return x;
}
