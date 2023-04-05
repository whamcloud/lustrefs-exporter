Name:           lustrefs_exporter
Version:        0.2.0
Release:        1%{?dist}
Summary:        prometheus exporter for lustre
License:        MIT

Requires(pre): shadow-utils

%description
Prometheus exporter for the Lustre filesystem

%global debug_package %{nil}

%prep

%build
cargo build --release

%install
install -v -d %{buildroot}%{_bindir}
install -v -d %{buildroot}%{_unitdir}
install -v -m 0644 lustrefs_exporter.service %{buildroot}%{_unitdir}
install -v target/release/lustrefs-exporter %{buildroot}%{_bindir}
%{__ln_s} lustrefs-exporter %{buildroot}%{_bindir}/lustrefs_exporter

%files
%{_bindir}/lustrefs-exporter
%{_bindir}/lustrefs_exporter
%{_unitdir}/lustrefs_exporter.service

%pre
getent group prometheus >/dev/null || groupadd -r prometheus
getent passwd prometheus >/dev/null || \
  useradd -r -g prometheus -s /sbin/nologin \
          -c "Prometheus services" prometheus

%post
%systemd_post %{name}.service

%preun
%systemd_preun %{name}.service

%postun
%systemd_postun %{name}.service
