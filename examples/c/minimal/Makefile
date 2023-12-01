CC     =  gcc
CFLAGS =  --std=c99 -O2 -g -DNDEBUG
CFLAGS += -Wall -Wextra -Wpedantic -Wcast-align -Wcast-qual -Wformat=2 -Wmissing-include-dirs -Wnull-dereference -Woverloaded-virtual -Wpointer-arith -Wshadow -Wswitch-enum -Wvla -Wno-sign-compare -Wconversion -Wunused -Wold-style-cast -Wno-missing-braces
CFLAGS += -I ../../../rerun_cpp/src/rerun/c # Make sure rerun.h is found
LDLIBS =

OBJECTS = main.o

ifeq ($(OS),Windows_NT)
	# TODO(emilk): Windows
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Linux)
		LDLIBS  += -lm
		OBJECTS += ../../../target/debug/librerun_c.a
    endif
    ifeq ($(UNAME_S),Darwin)
		# MacOS:
		LDLIBS  += -framework CoreFoundation -framework IOKit
		OBJECTS += ../../../target/debug/librerun_c.a
    endif
endif

all: example.bin

# Linking:
example.bin: rerun_c $(OBJECTS)
	$(CC) $(CFLAGS) $(LDLIBS) $(OBJECTS) -o example.bin

# Compiling:
main.o: main.c
	$(CC) $(CFLAGS) -c main.c -o main.o

# Always rebuild rerun_c; cargo will take care of not rebuilding unless needed
rerun_c: FORCE
	cargo build -p rerun_c

FORCE: ;

clean:
	rm -rf *.o *.bin

run: example.bin
	./example.bin
