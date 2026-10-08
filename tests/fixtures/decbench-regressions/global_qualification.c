int stream;

int global_qualification(int x)
{
    ::stream = x;
    if (::stream) return 1;
    return 0;
}
