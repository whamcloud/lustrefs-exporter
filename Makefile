.PHONY: rpm
rpm:
	$(MAKE) -C lustrefs-exporter rpm

.PHONY: deb
deb:
	$(MAKE) -C lustrefs-exporter deb
