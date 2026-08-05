import "i18next";
import enUS from "./locales/en-US.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    returnNull: false;
    resources: {
      translation: typeof enUS;
    };
  }
}
