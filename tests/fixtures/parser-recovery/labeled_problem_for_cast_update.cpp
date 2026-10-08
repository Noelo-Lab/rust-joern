int pointer_scalar(void *j){goto label;label:for(j=::stream;j;j=(void*)*((long long*)j+6))foo(j);return 1;}
int pointer_braced(void *j){goto label;label:for(j=::stream;j;j=(void*)*((long long*)j+6)){foo(j);}return 1;}
int pointer_unlabeled(void *j){for(j=::stream;j;j=(void*)*((long long*)j+6))foo(j);return 1;}
int pointer_multiline(void *j){
    goto label;
label:
    for(j=::stream;j;j=(void*)*((long long*)j+6))
        foo(j);
    return 1;
}
int pointer_multiline_braced(void *j){
    goto label;
label:
    for(j=::stream;j;j=(void*)*((long long*)j+6)){
        foo(j);
    }
    return 1;
}
int plain_multiline(void *j){
    goto label;
label:
    for(j=::stream;j;j=j+1)
        foo(j);
    return 1;
}
