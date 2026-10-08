struct item { char *name; int value; };
static struct item configuration = { "config", 0 };

int after_initializer(int x)
{
    if (x) return 1;
    return 0;
}
