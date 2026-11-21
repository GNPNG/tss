// Omitted sides use the existing wheel trajectory. Keep old projects sparse
// so simply opening and saving a wheel does not introduce a material override.
const grades = ['wheel', 'pdms-20', 'pdms-1000'];
export function validMaterialMotion(value) {
  if (value === undefined) return true;
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
  return Object.entries(value).every(([slide, grade]) =>
    (slide === 'rear' || slide === 'front') && grades.includes(grade));
}

export function materialMotionFields(layer) {
  const oils = layer?.materialMotion;
  if (oils === undefined) return {};
  return {materialMotion:{rear:oils.rear ?? 'wheel', front:oils.front ?? 'wheel'}};
}
