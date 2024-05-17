RPM_OPTS = -bb -D '_topdir ${CURDIR}/_rpm' -D '_sourcedir .' -D '_builddir .'

.PHONY: rpm
rpm:
	rpmbuild ${RPM_OPTS} lustrefs_exporter.spec


export DH_DESTDIR = ${CURDIR}/_deb

.PHONY: deb
deb:
	mkdir -p '${DH_DESTDIR}'
	./debian/rules clean
	fakeroot ./debian/rules binary
	./debian/rules clean

