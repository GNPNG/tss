// Omitted sides use the existing wheel trajectory. Keep old projects sparse
// so simply opening and saving a wheel does not introduce a material override.
const grades = ['wheel', 'pdms-20', 'pdms-1000'];
export function detailScaleFields(layer) {
  const scale = layer?.scale ?? layer?.detailScale ?? 1;
  return scale === 1 || layer?.media ? {} : {detailScale:scale};
}
export function simulationFields(layer) {
  return layer?.simulationSeed == null || layer.media ? {} :
    {simulation: {seed: layer.simulationSeed, rate: layer.speed}};
}
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
