void usage(int status) __noreturn
{
    if (!status)
    {
        fputs_unlocked(
            gettext("
                -h, --human-numeric-sort    compare human readable numbers (e.g., 2K 1G)\n"), 
            stdout);
    }
}

int after_usage(int value)
{
    if (value)
        return 1;
    return 0;
}
