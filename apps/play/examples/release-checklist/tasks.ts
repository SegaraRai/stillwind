export type Task = {
  id: number;
  title: string;
  owner: string;
  priority: "high" | "medium" | "low";
  done: boolean;
};

export const initialTasks: Task[] = [
  {
    id: 1,
    title: "Freeze the release branch",
    owner: "Aiko",
    priority: "high",
    done: true,
  },
  {
    id: 2,
    title: "Update the migration guide",
    owner: "Ben",
    priority: "medium",
    done: false,
  },
  {
    id: 3,
    title: "Record the demo video",
    owner: "Chen",
    priority: "low",
    done: false,
  },
  {
    id: 4,
    title: "Announce the release",
    owner: "Dana",
    priority: "high",
    done: false,
  },
];
