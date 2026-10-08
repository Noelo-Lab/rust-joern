int missing_goto_target(int x)
{
    if (x) goto absent;
    return 0;
}
