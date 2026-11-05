export const RUBROS: [string, string][] = [
  ["", "Todos los rubros"],
  ["bodega", "Bodegas y vino"],
  ["restaurante", "Restaurantes y cartas"],
  ["alojamiento", "Alojamientos turísticos"],
  ["automocion", "Automoción y talleres"],
  ["tanatorio", "Tanatorios y funerarias"],
  ["floristeria", "Floristerías"],
  ["granja_escuela", "Granjas escuela"],
  ["guarderia", "Guarderías"],
  ["veterinaria", "Veterinarias"],
  ["cuidado_mascotas", "Cuidado de mascotas"],
  ["fontaneria", "Fontanería"],
  ["electricidad", "Electricidad"],
  ["cerrajeria", "Cerrajería"],
  ["desatascos", "Desatascos"],
  ["control_plagas", "Control de plagas"],
  ["turismo_cultural", "Turismo y cultura"],
  ["asociacion_ong", "Asociaciones y ONGs"],
  ["otro", "Otros rubros"],
];

export const rubroNombre = (key: string | null | undefined) =>
  RUBROS.find(([value]) => value === key)?.[1] ?? key ?? "Sin rubro";

export const TIPOS_ENTIDAD: [string,string][] = [
  ["", "Todos los tipos de entidad"],
  ["cooperativa", "Cooperativas"],
  ["asociacion", "Asociaciones"],
  ["ong", "ONGs"],
  ["empresa_turistica", "Empresas turísticas"],
  ["empresa_industrial", "Empresas industriales"],
];
