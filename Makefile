#
# Core Flight Software CMake / GNU make wrapper
#
# ABOUT THIS MAKEFILE:
# It is a GNU-make wrapper that calls the CMake tools appropriately
# so that setting up a new build is fast and easy with no need to
# learn the CMake commands.  It also makes it easier to integrate
# the build with IDE tools such as Eclipse by providing a default
# makefile that has the common targets such as all/clean/etc.
#
# Use of this file is optional.
#
# This file is intended to be placed at the TOP-MOST level of the mission
# source tree, i.e. a level above "cfe".  Note this is outside the cfe
# repository which is why it cannot be delivered directly in place.
# To use it, simply copy it to the top directory.  As this just contains
# wrappers for the CMake targets, it is unlikely to change.  Projects
# are also free to customize this file and add their own targets after
# copying it to the top of the source tree.
#
# For _ALL_ targets defined in this file the build tree location may
# be specified via the "O" variable (i.e. make O=<my-build-dir> all).
# If not specified then the "build" subdirectory will be assumed.
#
# This wrapper defines the following major targets:
#  prep -- Runs CMake to create a new or re-configure an existing build tree
#    Note that multiple build trees can exist from a single source
#    Other control options (such as "SIMULATION") may be passed to CMake via
#    make variables depending on the mission build scripts.  These will be
#    cached in the build tree so they do not need to be set again thereafter.
#
#  all -- Build all targets in the CMake build tree
#
#  install -- Copy all files to the installation tree and run packaging scripts
#     The "DESTDIR" environment variable controls where the files are copied
#
#  clean -- Clean all targets in the CMake build tree, but not the build tree itself.
#
#  distclean -- Entirely remove the build directory specified by "O"
#      Note that after this the "prep" step must be run again in order to build.
#      Use caution with this as it does an rm -rf - don't set O to your home dir!
#
#  doc -- Build all doxygen source documentation.  The HTML documentation will be
#      generated under the build tree specified by "O".
#
#  usersguide -- Build all API/Cmd/Tlm doxygen documentation.  The HTML documentation
#      will be generated under the build tree specified by "O".
#
#  osalguide -- Build OSAL API doxygen documentation.  The HTML documentation will
#      be generated under the build tree specified by "O".
#
#  test -- Run all unit tests defined in the build.  Unit tests will typically only
#      be executable when building with the "SIMULATION=native" option.  Otherwise
#      it is up to the user to copy the executables to the target and run them.
#
#  lcov -- Runs the "lcov" tool on the build tree to collect all code coverage
#      analysis data and build the reports.  Code coverage data may be output by
#      the "make test" target above.
#

# Establish default values for critical variables.  Any of these may be overridden
# on the command line or via the make environment configuration in an IDE
O ?= build
ARCH ?= native/default_cpu1
BUILDTYPE ?= debug
INSTALLPREFIX ?= /exe
DESTDIR ?= $(O)

# The "DESTDIR" variable is a bit more complicated because it should be an absolute
# path for CMake, but we want to accept either absolute or relative paths.  So if
# the path does NOT start with "/", prepend it with the current directory.
ifeq ($(filter /%, $(DESTDIR)),)
DESTDIR := $(CURDIR)/$(DESTDIR)
endif

# The "LOCALTGTS" defines the top-level targets that are implemented in this makefile
# Any other target may also be given, in that case it will simply be passed through.
LOCALTGTS := doc usersguide osalguide prep all clean install distclean test lcov check check-nos3 docker-build docker-prep docker-all docker-install docker-run docker-shell docker-test constellation-build constellation-gen constellation-up constellation-down nos3-prep wildfire-demo-build wildfire-demo-up wildfire-demo-down nos3-build nos3-config nos3-build-fsw nos3-build-sim nos3-launch nos3-stop nos3-shell eosim-gen
OTHERTGTS := $(filter-out $(LOCALTGTS),$(MAKECMDGOALS))

# As this makefile does not build any real files, treat everything as a PHONY target
# This ensures that the rule gets executed even if a file by that name does exist
.PHONY: $(LOCALTGTS) $(OTHERTGTS)

# If the target name appears to be a directory (ends in /), do a make all in that directory
DIRTGTS := $(filter %/,$(OTHERTGTS))
ifneq ($(DIRTGTS),)
$(DIRTGTS):
	$(MAKE) -C $(O)/$(patsubst $(O)/%,%,$(@)) all
endif

# For any other goal that is not one of the known local targets, pass it to the arch build
# as there might be a target by that name.  For example, this is useful for rebuilding
# single unit test executable files while debugging from the IDE
FILETGTS := $(filter-out $(DIRTGTS),$(OTHERTGTS))
ifneq ($(FILETGTS),)
$(FILETGTS):
	$(MAKE) -C $(O)/$(ARCH) $(@)
endif

# The "prep" step requires extra options that are specified via environment variables.
# Certain special ones should be passed via cache (-D) options to CMake.
# These are only needed for the "prep" target but they are computed globally anyway.
PREP_OPTS :=

ifneq ($(INSTALLPREFIX),)
PREP_OPTS += -DCMAKE_INSTALL_PREFIX=$(INSTALLPREFIX)
endif

ifneq ($(VERBOSE),)
PREP_OPTS += --trace
endif

ifneq ($(BUILDTYPE),)
PREP_OPTS += -DCMAKE_BUILD_TYPE=$(BUILDTYPE)
endif

ifneq ($(CMAKE_PREFIX_PATH),)
PREP_OPTS += -DCMAKE_PREFIX_PATH=$(CMAKE_PREFIX_PATH)
endif

all:
	$(MAKE) --no-print-directory -C "$(O)" mission-all

install:
	$(MAKE) --no-print-directory -C "$(O)" DESTDIR="$(DESTDIR)" mission-install

prep $(O)/.prep:
	mkdir -p "$(O)"
	(cd "$(O)" && cmake $(PREP_OPTS) "$(CURDIR)/cfe")
	echo "$(PREP_OPTS)" > "$(O)/.prep"

clean:
	$(MAKE) --no-print-directory -C "$(O)" mission-clean

distclean:
	rm -rf "$(O)"

# Grab lcov baseline before running tests
test:
	lcov --capture --initial --directory $(O)/$(ARCH) --output-file $(O)/$(ARCH)/coverage_base.info
	(cd $(O)/$(ARCH) && ctest -O ctest.log)

lcov:
	lcov --capture --rc lcov_branch_coverage=1 --directory $(O)/$(ARCH) --output-file $(O)/$(ARCH)/coverage_test.info
	lcov --rc lcov_branch_coverage=1 --add-tracefile $(O)/$(ARCH)/coverage_base.info --add-tracefile $(O)/$(ARCH)/coverage_test.info --output-file $(O)/$(ARCH)/coverage_total.info
	genhtml $(O)/$(ARCH)/coverage_total.info --branch-coverage --output-directory $(O)/$(ARCH)/lcov
	@/bin/echo -e "\n\nCoverage Report Link: file:$(CURDIR)/$(O)/$(ARCH)/lcov/index.html\n"

doc:
	$(MAKE) --no-print-directory -C "$(O)" mission-doc

usersguide:
	$(MAKE) --no-print-directory -C "$(O)" cfe-usersguide

osalguide:
	$(MAKE) --no-print-directory -C "$(O)" osal-apiguide

# Make all the commands that use the build tree depend on a flag file
# that is used to indicate the prep step has been done.  This way
# the prep step does not need to be done explicitly by the user
# as long as the default options are sufficient.
$(filter-out prep distclean check check-nos3 docker-build docker-prep docker-all docker-install docker-run docker-shell docker-test constellation-build constellation-gen constellation-up constellation-down nos3-prep wildfire-demo-build wildfire-demo-up wildfire-demo-down nos3-build nos3-config nos3-build-fsw nos3-build-sim nos3-launch nos3-stop nos3-shell,$(LOCALTGTS)): $(O)/.prep

# Docker targets for building on macOS
docker-build:
	docker compose build

docker-prep:
	docker compose run --rm cfs-build bash -c "make SIMULATION=native prep"

docker-all:
	docker compose run --rm cfs-build make

docker-install:
	docker compose run --rm cfs-build make install

docker-run:
	docker compose run --service-ports --rm cfs-build bash -c "cd build/exe/cpu1 && ./core-cpu1"

docker-shell:
	docker compose run --rm cfs-build bash

docker-test:
	docker compose run --rm cfs-test bash -c \
		"[ -f build/.prep ] || make SIMULATION=native prep && cd crates/leodos-protocols && cargo test --features=cfs"

check:
	cd crates/leodos-protocols && cargo check --features=cfs && cargo test --features=cfs
	cd crates/leodos-libcfs && cargo check

check-nos3: nos3-prep
	$(NOS3_RUN_STANDALONE) bash -c "cd apps/wildfire/fsw && cargo check"

# Constellation targets
MAX_ORB ?= 3
MAX_SAT ?= 3

constellation-build:
	docker build -f tools/constellation/Dockerfile.sat -t leodos-sat:latest .

constellation-up:
	docker run --rm -it \
		--name leodos-constellation \
		-e MAX_ORB=$(MAX_ORB) \
		-e MAX_SAT=$(MAX_SAT) \
		-p 1234:1234/udp \
		-p 1235:1235/udp \
		--sysctl fs.mqueue.msg_max=1000 \
		leodos-sat:latest

constellation-down:
	docker stop leodos-constellation 2>/dev/null || true

# Wildfire demo (NOS3 simulation with thermal camera + GPS)
NOS3_RUN_STANDALONE = docker run --rm -v "$$(pwd):/cFS" -v "$${HOME}/.nos3:/root/.nos3" -w /cFS nos3-rust:latest

nos3-prep:
	docker build -f Dockerfile.nos3 -t nos3-rust:latest .
	$(NOS3_RUN_STANDALONE) bash -c "cd libs/nos3 && make config && make build-fsw"

wildfire-demo-build: nos3-prep
	$(NOS3_RUN_STANDALONE) bash -c "cd libs/42 && make clean && make 42PLATFORM=__linux__ GUIFLAG= SHADERFLAG= && mkdir -p /root/.nos3/42/NOS3InOut && tar --exclude=.git -cf - . | tar -xf - -C /root/.nos3/42/ && cp -r /cFS/libs/nos3/cfg/build/InOut/* /root/.nos3/42/NOS3InOut/"
	$(NOS3_RUN_STANDALONE) bash -c "cd libs/nos3 && make build-sim && make build-fsw"

wildfire-demo-up:
	$(NOS3_DC) up -d

wildfire-demo-down:
	$(NOS3_DC) down

# NOS3 simulation targets
NOS3_DC = docker compose -f docker-compose.nos3.yml
NOS3_RUN = $(NOS3_DC) run --rm fsw

nos3-build:
	docker build -f Dockerfile.nos3 -t nos3-rust:latest .

nos3-config:
	$(NOS3_RUN) bash -c "cd libs/nos3 && make config"

nos3-build-fsw:
	$(NOS3_RUN) bash -c "cd libs/nos3 && make build-fsw"

nos3-build-sim:
	$(NOS3_RUN) bash -c "cd libs/nos3 && make build-sim"

nos3-launch:
	$(NOS3_DC) up -d

nos3-stop:
	$(NOS3_DC) down

nos3-shell:
	$(NOS3_DC) run --rm fsw bash

# Synthetic sensor data generation (eosim)
EOSIM_DIR = tools/eosim
EOSIM_SCENARIO ?= $(EOSIM_DIR)/examples/california_wildfire.yaml
EOSIM_OUTPUT = $(EOSIM_DIR)/output

eosim-gen:
	cd $(EOSIM_DIR) && uv run eosim wildfire $(abspath $(EOSIM_SCENARIO)) -o $(abspath $(EOSIM_OUTPUT)) --fmt bin
