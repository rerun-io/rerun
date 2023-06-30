CC     =  g++
CFLAGS =  --std=c++14 -Wall -Wno-sign-compare -O2 -g -DNDEBUG
CFLAGS += -I ../src # Make sure rerun.h is found
LDLIBS =  -lstdc++ -lpthread -ldl

OBJECTS = main.o

ifeq ($(OS),Windows_NT)
	# TODO(emilk): Windows
else
    UNAME_S := $(shell uname -s)
    ifeq ($(UNAME_S),Linux)
        $ TODO(emilk): Linux
    endif
    ifeq ($(UNAME_S),Darwin)
		# MacOS:
		LDLIBS  += -framework CoreFoundation -framework IOKit
		OBJECTS += ../../../target/debug/librerun_c.a
    endif
endif

all: example.bin rerun_c

# Linking:
example.bin: $(OBJECTS)
	$(CC) $(CFLAGS) $(OBJECTS) $(LDLIBS) -o example.bin

# Compiling:
main.o: main.cpp
	$(CC) $(CFLAGS) -c main.cpp -o main.o

# Always rebuild rerun_c; cargo will take care of not rebuilding unless needed
rerun_c: FORCE
	cargo build -p rerun_c

FORCE: ;

clean:
	rm -rf *.o *.bin
