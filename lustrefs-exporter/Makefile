DEFAULT_VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/ \
		{s/^version *= *"\(.*\)"/\1/p}' ../Cargo.toml)

ifeq (${DEFAULT_VERSION},)
    $(error no version found in ../Cargo.toml)
endif

GIT_VERSION := $(shell git describe --tags --dirty --match 'v[0-9]*' --abbrev=7 \
		2>/dev/null | sed 's/^v//')

PKG_VERSION := $(or ${GIT_VERSION},${DEFAULT_VERSION})

ifneq ($(filter-out ${DEFAULT_VERSION}%,${PKG_VERSION}),)
    $(error no v${DEFAULT_VERSION} tag found, run 'git fetch --tags')
endif

RPM_OPTS = -bb -D '_topdir ${CURDIR}/_rpm' -D '_sourcedir .' -D '_builddir .' \
	   -D 'rpm_pkg_version $(subst -,_,${PKG_VERSION})'

.PHONY: rpm
rpm:
	rpmbuild ${RPM_OPTS} lustrefs_exporter.spec


export DH_DESTDIR = ${CURDIR}/_deb
export DEB_PKG_VERSION = $(subst -,+,${PKG_VERSION})

.PHONY: deb
deb:
	mkdir -p '${DH_DESTDIR}'
	./debian/rules clean
	fakeroot ./debian/rules binary
	./debian/rules clean

