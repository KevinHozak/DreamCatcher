# Native remote-image review

## S010 historical behavior

S010 added the generated-only native review flow and offline one-use confirmation. The 1.9.4 Windows qualification recorded the generated preview, edit/redaction, keyboard and cancellation checks. At that stage remote sending stayed disabled; those results describe that historical source and build only.

## S030 shared-host source behavior

DreamCatcher 1.9.7 adds an explicit shared Portal path for the generated 32x32 image only. Preview and status remain metadata-only and do not acquire/start a host, read credentials, or access media. The ordinary local classifier remains the default, and selected-media review remains unavailable.

An explicit **Send remotely** action consumes the current native review, acquires the registered Portal shared host, and revalidates that review before starting the pinned native transport broker. The broker sends the bounded generated-only request over the verified host generation. It does not construct a Gateway, own provider routing/accounting/consent, read the aggregate service credential file, or stop the independently supervised host. Final route eligibility, the visible native consent dialog, and dispatch are owned by the shared Portal Gateway. Cancellation stops only this request or its broker; ambiguous outcomes are not replayed.

The fixture-only `portal-fixtures` build and `--portal-shared-fixture-qualification` entry point require a complete generated fixture configuration before Tauri initializes. They open an isolated generated-only React view and use a supervised fixture host, dummy credentials, and local synthetic transport. They are not enabled in production builds and do not qualify a live provider route. The existing `--portal-consent-qualification` mode remains offline-only with Send disabled. No profile conversion, key provisioning, service change, route enablement, or release deployment is part of S030.

## Verification boundaries

The native code pins the registered host runtime/bootstrap and uses the pinned Portal SDK; renderer input cannot provide filesystem paths, URLs, keys, host generations, or operation IDs. Review identity, payload fingerprint, expiry, and cancellation are checked again after host acquisition and before broker dispatch. Text and classification are transient UI data, not persisted to jobs or media caches.

Production route qualification still requires separate Portal eligibility, privacy, billing, and operator review. S030 source integration does not claim a production route is enabled or that any paid provider request was made. The prior 1.9.6 installed release and its normal app data remain unchanged.

## Offline checks

Use the repository's cached dependencies only. The fixture transport tests must use generated roots and synthetic app/provider credentials. Do not launch the normal application for qualification because its standard startup can inspect user inventory/settings. Actual WebView/native-dialog acceptance is recorded separately from Rust unit tests and Python synthetic-host tests.