import { useTranslation } from "react-i18next";
import { AppLink } from "../app/navigation";
import { APP_VERSION } from "../app/version";

const helpSections = [
  "available",
  "unavailable",
  "quickStart",
  "files",
  "deepseek",
  "privacy",
  "faq",
  "feedback",
  "guides",
] as const;

export function HelpPage() {
  const { t } = useTranslation();

  return (
    <section className="page-stack help-page" data-acceptance="help-page">
      <header className="page-heading">
        <div>
          <p className="eyebrow">{t("help.eyebrow")}</p>
          <h1>{t("help.title")}</h1>
          <p>{t("help.intro")}</p>
        </div>
      </header>

      <article className="content-panel">
        <h2>{t("help.versionTitle")}</h2>
        <p>{t("help.versionBody", { version: APP_VERSION })}</p>
      </article>

      <div className="help-grid">
        {helpSections.map((section) => (
          <article className="content-panel" key={section}>
            <h2>{t(`help.${section}Title`)}</h2>
            <p>{t(`help.${section}Body`)}</p>
          </article>
        ))}
      </div>

      <AppLink className="button button--secondary" to="/settings">
        {t("app.nav.settings")}
      </AppLink>
    </section>
  );
}
