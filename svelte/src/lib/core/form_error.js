// The error a form's call throws in a browser and in the app alike: the
// message, and `errors`, the fields that were wrong, `{field: [message]}`,
// as app_core's FieldErrors answers them; {} when none were.

export class FormError extends Error {
  /**
   * @param {string} message
   * @param {Record<string, string[]>} [errors]
   */
  constructor(message, errors = {}) {
    super(message);
    this.name = "FormError";
    this.errors = errors;
  }
}
